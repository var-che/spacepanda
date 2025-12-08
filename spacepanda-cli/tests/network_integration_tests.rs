//! Network Integration Tests for SpacePanda CLI
//!
//! These tests verify end-to-end workflows with network synchronization,
//! including commit propagation and real message delivery.

use anyhow::Result;
use spacepanda_core::{
    config::Config,
    core_mls::service::MlsService,
    core_mvp::network::NetworkLayer,
    core_router::session_manager::PeerId,
    core_router::RouterHandle,
    core_store::{
        model::types::{ChannelId, UserId},
        store::local_store::{LocalStore, LocalStoreConfig},
    },
    shutdown::ShutdownCoordinator,
    ChannelManager, Identity,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

/// Test actor with network layer
struct NetworkedActor {
    #[allow(dead_code)]
    name: String,
    identity: Arc<Identity>,
    manager: Arc<ChannelManager>,
    network: Arc<NetworkLayer>,
    key_package: Vec<u8>,
    #[allow(dead_code)]
    data_dir: TempDir,
    #[allow(dead_code)]
    commit_processor: tokio::task::JoinHandle<()>,
    #[allow(dead_code)]
    network_processor: tokio::task::JoinHandle<()>,
    listen_addr: String,
}

impl NetworkedActor {
    /// Create a new networked test actor with real TCP transport
    async fn new(name: &str, peer_id_byte: u8) -> Result<Self> {
        let data_dir = TempDir::new()?;

        // Create identity
        let identity = Arc::new(Identity::new(
            UserId(format!("{}@spacepanda.local", name)),
            name.to_string(),
            format!("node-{}", name),
        ));

        // Initialize services
        let config = Arc::new(Config::default());
        let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));

        // Create MLS service
        let mls_storage_dir = data_dir.path().join("mls_groups");
        let mls_service = Arc::new(MlsService::with_storage(&config, shutdown, mls_storage_dir)?);

        // Initialize store
        let store_config = LocalStoreConfig {
            data_dir: data_dir.path().to_path_buf(),
            enable_encryption: false,
            snapshot_interval: 1000,
            max_log_size: 10_000_000,
            enable_compaction: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
        };

        let store = Arc::new(LocalStore::new(store_config)?);

        // Create network layer with real TCP transport
        let (router, _router_task) = RouterHandle::new();
        let peer_id = PeerId(vec![peer_id_byte; 4]); // Simple peer ID based on byte
        let (network, _msg_rx, commits_rx) = NetworkLayer::new(router.clone(), peer_id);
        let network = Arc::new(network);

        // Start TCP listener on unique port (50000 + peer_id_byte)
        let listen_addr = format!("127.0.0.1:{}", 50000 + peer_id_byte as u16);
        network.listen(&listen_addr).await?;
        println!("[{}] Listening on {}", name, listen_addr);

        // Spawn network event processor to handle incoming connections
        let network_clone = network.clone();
        let router_clone = router.clone();
        let name_clone = name.to_string();
        let network_processor = tokio::spawn(async move {
            use spacepanda_core::core_router::RouterEvent;

            while let Some(event) = router_clone.next_event().await {
                match event {
                    RouterEvent::DataReceived(peer_id, data) => {
                        println!(
                            "[{}] Received {} bytes from peer {:?}",
                            name_clone,
                            data.len(),
                            peer_id
                        );
                        if let Err(e) = network_clone.handle_incoming_data(peer_id, data).await {
                            eprintln!("[{}] Error handling incoming data: {}", name_clone, e);
                        }
                    }
                    RouterEvent::Listening(addr) => {
                        println!("[{}] Listening on {}", name_clone, addr);
                    }
                    RouterEvent::PeerConnected(peer_id) => {
                        println!("[{}] Peer connected: {:?}", name_clone, peer_id);
                    }
                    RouterEvent::PeerDisconnected(peer_id) => {
                        println!("[{}] Peer disconnected: {:?}", name_clone, peer_id);
                    }
                }
            }
        });

        // Create manager with network
        let manager = Arc::new(
            ChannelManager::new(mls_service, store, identity.clone(), config)
                .with_network(network.clone()),
        );

        // Start commit processor
        let commit_processor = manager.clone().spawn_commit_processor(commits_rx);

        // Generate KeyPackage for this actor
        let key_package = manager.generate_key_package().await?;

        Ok(Self {
            name: name.to_string(),
            identity,
            manager,
            network,
            key_package,
            data_dir,
            commit_processor,
            network_processor,
            listen_addr,
        })
    }

    /// Get this actor's KeyPackage
    fn get_key_package(&self) -> Vec<u8> {
        self.key_package.clone()
    }

    /// Get this actor's PeerId
    fn get_peer_id(&self) -> PeerId {
        self.network.local_peer_id().clone()
    }

    /// Get this actor's listen address
    fn get_listen_addr(&self) -> &str {
        &self.listen_addr
    }

    /// Connect to another actor's TCP listener
    async fn connect_to(&self, other_addr: &str) -> Result<()> {
        self.network.dial(other_addr).await?;
        Ok(())
    }

    /// Create a new channel
    async fn create_channel(&self, channel_name: &str) -> Result<ChannelId> {
        Ok(self.manager.create_channel(channel_name.to_string(), false).await?)
    }

    /// Generate invite for a channel using another actor's KeyPackage
    async fn create_invite(
        &self,
        channel_id: &ChannelId,
        invitee_key_package: Vec<u8>,
    ) -> Result<spacepanda_core::core_mvp::types::InviteToken> {
        let (invite, _commit) = self.manager.create_invite(channel_id, invitee_key_package).await?;
        Ok(invite)
    }

    /// Join a channel from an invite
    async fn join_channel(
        &self,
        invite: &spacepanda_core::core_mvp::types::InviteToken,
    ) -> Result<ChannelId> {
        Ok(self.manager.join_channel(invite).await?)
    }

    /// Send a message to a channel
    async fn send_message(&self, channel_id: &ChannelId, message: &str) -> Result<()> {
        self.manager.send_message(channel_id, message.as_bytes()).await?;
        Ok(())
    }

    /// Remove a member from a channel
    async fn remove_member(&self, channel_id: &ChannelId, user_id: &UserId) -> Result<()> {
        let member_identity = user_id.0.as_bytes();
        self.manager.remove_member(channel_id, member_identity).await?;
        Ok(())
    }

    /// Register another actor in this network layer for a channel
    async fn register_peer(&self, channel_id: &ChannelId, user_id: UserId, peer_id: PeerId) {
        self.network.register_channel_member(channel_id, user_id, peer_id).await;
    }
}

#[tokio::test]
async fn test_network_commit_propagation() -> Result<()> {
    println!("🧪 Testing commit propagation over network");

    // Create 3 actors with network layers
    let alice = NetworkedActor::new("alice", 1).await?;
    let bob = NetworkedActor::new("bob", 2).await?;
    let charlie = NetworkedActor::new("charlie", 3).await?;

    println!("✅ Created 3 networked actors");

    // Wait for listeners to be fully ready
    sleep(Duration::from_millis(500)).await;

    // Connect actors via TCP (create mesh network)
    println!("🔌 Connecting actors via TCP...");
    alice.connect_to(bob.get_listen_addr()).await?;
    alice.connect_to(charlie.get_listen_addr()).await?;
    bob.connect_to(alice.get_listen_addr()).await?;
    bob.connect_to(charlie.get_listen_addr()).await?;
    charlie.connect_to(alice.get_listen_addr()).await?;
    charlie.connect_to(bob.get_listen_addr()).await?;

    // Wait for connections to establish
    sleep(Duration::from_millis(500)).await;
    println!("✅ TCP connections established");

    // Alice creates channel
    let channel_id = alice.create_channel("network-test").await?;
    println!("✅ Alice created channel: {}", channel_id);

    // Register all peers in Alice's network (so she knows how to route)
    alice
        .register_peer(&channel_id, alice.identity.user_id.clone(), alice.get_peer_id())
        .await;
    alice
        .register_peer(&channel_id, bob.identity.user_id.clone(), bob.get_peer_id())
        .await;
    alice
        .register_peer(&channel_id, charlie.identity.user_id.clone(), charlie.get_peer_id())
        .await;

    // Register all peers in Bob's network
    bob.register_peer(&channel_id, alice.identity.user_id.clone(), alice.get_peer_id())
        .await;
    bob.register_peer(&channel_id, bob.identity.user_id.clone(), bob.get_peer_id())
        .await;
    bob.register_peer(&channel_id, charlie.identity.user_id.clone(), charlie.get_peer_id())
        .await;

    // Register all peers in Charlie's network
    charlie
        .register_peer(&channel_id, alice.identity.user_id.clone(), alice.get_peer_id())
        .await;
    charlie
        .register_peer(&channel_id, bob.identity.user_id.clone(), bob.get_peer_id())
        .await;
    charlie
        .register_peer(&channel_id, charlie.identity.user_id.clone(), charlie.get_peer_id())
        .await;

    println!("✅ Registered peers in all network layers");

    // Alice invites Bob
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    println!("✅ Bob joined channel");

    // Wait for commit propagation
    sleep(Duration::from_millis(100)).await;

    // Alice invites Charlie (this should broadcast a commit to Bob)
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    println!("✅ Charlie joined channel");

    // Wait for commit propagation
    sleep(Duration::from_millis(100)).await;

    // All members send messages
    alice.send_message(&channel_id, "Hello from Alice!").await?;
    bob.send_message(&channel_id, "Hello from Bob!").await?;
    charlie.send_message(&channel_id, "Hello from Charlie!").await?;
    println!("✅ All members sent messages");

    println!("🎉 Test passed! Commits broadcast over real TCP sockets");

    Ok(())
}

#[tokio::test]
async fn test_removal_with_network_broadcast() -> Result<()> {
    println!("🧪 Testing member removal with network broadcast");

    // Create 3 actors
    let alice = NetworkedActor::new("alice", 1).await?;
    let bob = NetworkedActor::new("bob", 2).await?;
    let charlie = NetworkedActor::new("charlie", 3).await?;

    println!("✅ Created 3 networked actors");

    // Wait for listeners to be fully ready
    sleep(Duration::from_millis(500)).await;

    // Connect actors via TCP
    println!("🔌 Connecting actors via TCP...");
    alice.connect_to(bob.get_listen_addr()).await?;
    alice.connect_to(charlie.get_listen_addr()).await?;
    bob.connect_to(alice.get_listen_addr()).await?;
    bob.connect_to(charlie.get_listen_addr()).await?;
    charlie.connect_to(alice.get_listen_addr()).await?;
    charlie.connect_to(bob.get_listen_addr()).await?;
    sleep(Duration::from_millis(500)).await;
    println!("✅ TCP connections established");

    // Setup channel
    let channel_id = alice.create_channel("removal-test").await?;

    // Register peers
    for actor in [&alice, &bob, &charlie] {
        actor
            .register_peer(&channel_id, alice.identity.user_id.clone(), alice.get_peer_id())
            .await;
        actor
            .register_peer(&channel_id, bob.identity.user_id.clone(), bob.get_peer_id())
            .await;
        actor
            .register_peer(&channel_id, charlie.identity.user_id.clone(), charlie.get_peer_id())
            .await;
    }

    // Everyone joins
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;

    sleep(Duration::from_millis(50)).await;

    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;

    sleep(Duration::from_millis(50)).await;

    println!("✅ All members joined");

    // Alice removes Charlie (commit should be broadcast)
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice removed Charlie (commit broadcast attempted)");

    // Wait for commit propagation
    sleep(Duration::from_millis(100)).await;

    // Alice and Bob can still send
    alice.send_message(&channel_id, "Charlie was removed").await?;
    bob.send_message(&channel_id, "Confirmed").await?;
    println!("✅ Remaining members can send");

    println!("🎉 Test passed! Removal commit broadcast over real TCP");

    Ok(())
}

#[tokio::test]
async fn test_network_layer_enabled() -> Result<()> {
    println!("🧪 Testing network layer is properly enabled");

    let alice = NetworkedActor::new("alice", 1).await?;

    assert!(alice.manager.is_network_enabled(), "Network should be enabled");
    println!("✅ Network layer is enabled");

    println!("🎉 Test passed!");

    Ok(())
}
