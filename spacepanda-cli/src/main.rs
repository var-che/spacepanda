use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use spacepanda_core::{
    config::Config,
    core_mls::service::MlsService,
    core_store::store::local_store::{LocalStore, LocalStoreConfig},
    logging::{init_logging_with_config, LogConfig, LogLevel},
    shutdown::ShutdownCoordinator,
    ChannelManager, Identity,
};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "spacepanda")]
#[command(author, version, about = "Privacy-first encrypted chat", long_about = None)]
struct Args {
    /// Set the log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable JSON formatted logging
    #[arg(long)]
    json_logs: bool,

    /// Data directory for storage
    #[arg(short, long, default_value = "~/.spacepanda")]
    data_dir: String,

    /// Subcommand to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize SpacePanda (create identity and storage)
    Init {
        /// Your display name
        #[arg(short, long)]
        name: String,
    },
    
    /// Channel management commands
    #[command(subcommand)]
    Channel(ChannelCommand),
    
    /// Send an encrypted message
    Send {
        /// Channel ID to send to
        channel_id: String,
        
        /// Message to send
        message: String,
    },
    
    /// Listen for incoming messages (interactive mode)
    Listen {
        /// Channel ID to listen on
        channel_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum ChannelCommand {
    /// Create a new encrypted channel
    Create {
        /// Channel name
        name: String,
        
        /// Make channel publicly discoverable
        #[arg(long)]
        public: bool,
    },
    
    /// Join a channel from an invite code
    Join {
        /// Base64-encoded invite token
        invite: String,
    },
    
    /// Generate an invite code for a channel
    Invite {
        /// Channel ID to create invite for
        channel_id: String,
    },
    
    /// List all your channels
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse log level
    let log_level = LogLevel::from_str(&args.log_level).unwrap_or_else(|| {
        eprintln!("Invalid log level '{}', using 'info'", args.log_level);
        LogLevel::Info
    });

    // Initialize logging
    let config = LogConfig::new(log_level).json_format(args.json_logs);
    init_logging_with_config(config)?;

    info!("SpacePanda CLI started");

    // Expand tilde in data dir
    let data_dir = shellexpand::tilde(&args.data_dir).to_string();
    let data_path = PathBuf::from(&data_dir);

    // Execute command
    match args.command {
        Command::Init { name } => {
            cmd_init(&data_path, &name).await?;
        }
        Command::Channel(channel_cmd) => {
            let manager = load_manager(&data_path).await?;
            match channel_cmd {
                ChannelCommand::Create { name, public } => {
                    cmd_channel_create(manager, &name, public).await?;
                }
                ChannelCommand::Join { invite } => {
                    cmd_channel_join(manager, &invite).await?;
                }
                ChannelCommand::Invite { channel_id } => {
                    cmd_channel_invite(manager, &channel_id).await?;
                }
                ChannelCommand::List => {
                    cmd_channel_list(manager).await?;
                }
            }
        }
        Command::Send { channel_id, message } => {
            let manager = load_manager(&data_path).await?;
            cmd_send(manager, &channel_id, &message).await?;
        }
        Command::Listen { channel_id } => {
            let manager = load_manager(&data_path).await?;
            cmd_listen(manager, &channel_id).await?;
        }
    }

    info!("SpacePanda CLI finished");
    Ok(())
}

/// Initialize SpacePanda (create identity and local storage)
async fn cmd_init(data_dir: &PathBuf, name: &str) -> Result<()> {
    info!("Initializing SpacePanda at {:?}", data_dir);
    
    // Create data directory
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("Failed to create directory {:?}", data_dir))?;
    
    // Create identity file
    let identity_path = data_dir.join("identity.json");
    if identity_path.exists() {
        return Err(anyhow::anyhow!(
            "Identity already exists at {:?}. Remove it to reinitialize.", 
            identity_path
        ));
    }
    
    // Generate new identity
    let identity = Identity::new(
        spacepanda_core::core_store::model::types::UserId(uuid::Uuid::new_v4().to_string()),
        name.to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    
    // Save identity
    let identity_json = serde_json::to_string_pretty(&identity)?;
    std::fs::write(&identity_path, identity_json)?;
    
    // Initialize local store
    let store_config = LocalStoreConfig {
        data_dir: data_dir.clone(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    
    let _store = LocalStore::new(store_config)
        .with_context(|| "Failed to initialize local store")?;
    
    println!("✅ SpacePanda initialized successfully!");
    println!("   Data directory: {:?}", data_dir);
    println!("   User ID: {}", identity.user_id);
    println!("   Display name: {}", identity.display_name);
    println!("\nNext steps:");
    println!("  - Create a channel: spacepanda channel create <name>");
    println!("  - Join a channel: spacepanda channel join <invite-code>");
    
    Ok(())
}

/// Load ChannelManager from data directory
async fn load_manager(data_dir: &PathBuf) -> Result<Arc<ChannelManager>> {
    // Load identity
    let identity_path = data_dir.join("identity.json");
    if !identity_path.exists() {
        return Err(anyhow::anyhow!(
            "Identity not found. Run 'spacepanda init' first."
        ));
    }
    
    let identity_json = std::fs::read_to_string(&identity_path)?;
    let identity: Identity = serde_json::from_str(&identity_json)?;
    
    // Initialize services
    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
    
    // Create MLS service with storage persistence
    let mls_storage_dir = data_dir.join("mls_groups");
    let mls_service = Arc::new(
        MlsService::with_storage(&config, shutdown, mls_storage_dir)
            .with_context(|| "Failed to initialize MLS service with storage")?
    );
    
    // Initialize store
    let store_config = LocalStoreConfig {
        data_dir: data_dir.clone(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    
    let store = Arc::new(LocalStore::new(store_config)?);
    
    // Create manager
    let manager = Arc::new(ChannelManager::new(
        mls_service,
        store,
        Arc::new(identity),
        config,
    ));
    
    Ok(manager)
}

/// Create a new encrypted channel
async fn cmd_channel_create(manager: Arc<ChannelManager>, name: &str, public: bool) -> Result<()> {
    info!("Creating channel: {}", name);
    
    let channel_id = manager.create_channel(name.to_string(), public).await?;
    
    println!("✅ Channel created successfully!");
    println!("   Channel ID: {}", channel_id);
    println!("   Name: {}", name);
    println!("   Public: {}", if public { "yes" } else { "no" });
    println!("\nTo invite others:");
    println!("  spacepanda channel invite {}", channel_id);
    
    Ok(())
}

/// Join a channel from an invite code
async fn cmd_channel_join(manager: Arc<ChannelManager>, invite_b64: &str) -> Result<()> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    
    info!("Joining channel from invite");
    
    // Decode invite from base64
    let invite_bytes = STANDARD.decode(invite_b64)
        .with_context(|| "Invalid invite code (not valid base64)")?;
    
    let invite: spacepanda_core::core_mvp::types::InviteToken = 
        serde_json::from_slice(&invite_bytes)
            .with_context(|| "Invalid invite format")?;
    
    let channel_id = manager.join_channel(&invite).await?;
    
    println!("✅ Successfully joined channel!");
    println!("   Channel ID: {}", channel_id);
    println!("   Name: {}", invite.channel_name);
    println!("\nYou can now send messages:");
    println!("  spacepanda send {} \"Hello!\"", channel_id);
    
    Ok(())
}

/// Generate an invite code for a channel
async fn cmd_channel_invite(manager: Arc<ChannelManager>, channel_id_str: &str) -> Result<()> {
    use spacepanda_core::core_store::model::types::ChannelId;
    
    info!("Creating invite for channel: {}", channel_id_str);
    
    let channel_id = ChannelId(channel_id_str.to_string());
    
    // Generate a key package for the invitee (they'll need to import this)
    // For MVP, we create a temporary key package
    // TODO: In production, the invitee should generate their own key package
    warn!("Generating temporary key package (MVP limitation)");
    let key_package = manager.generate_key_package().await?;
    
    let (invite, _commit) = manager.create_invite(&channel_id, key_package).await?;
    
    // Encode invite as base64
    use base64::{Engine, engine::general_purpose::STANDARD};
    let invite_bytes = serde_json::to_vec(&invite)?;
    let invite_b64 = STANDARD.encode(&invite_bytes);
    
    println!("✅ Invite created successfully!");
    println!("\nInvite code:");
    println!("{}", invite_b64);
    println!("\nShare this with the person you want to invite.");
    println!("They can join with:");
    println!("  spacepanda channel join <invite-code>");
    
    Ok(())
}

/// List all channels
async fn cmd_channel_list(manager: Arc<ChannelManager>) -> Result<()> {
    let channels = manager.list_channels().await?;
    
    if channels.is_empty() {
        println!("No channels found.");
        println!("\nCreate a new channel:");
        println!("  spacepanda channel create <name>");
        return Ok(());
    }
    
    println!("Your channels:\n");
    for channel in channels {
        println!("  📁 {} ({})", channel.name, channel.channel_id);
        println!("     Owner: {}", channel.owner);
        println!("     Public: {}", if channel.is_public { "yes" } else { "no" });
        println!();
    }
    
    Ok(())
}

/// Send an encrypted message
async fn cmd_send(manager: Arc<ChannelManager>, channel_id_str: &str, message: &str) -> Result<()> {
    use spacepanda_core::core_store::model::types::ChannelId;
    
    let channel_id = ChannelId(channel_id_str.to_string());
    
    info!("Sending message to channel: {}", channel_id);
    
    let _ciphertext = manager.send_message(&channel_id, message.as_bytes()).await?;
    
    println!("✅ Message sent successfully!");
    
    Ok(())
}

/// Listen for incoming messages (interactive mode)
async fn cmd_listen(_manager: Arc<ChannelManager>, channel_id: &str) -> Result<()> {
    println!("🎧 Listening on channel: {}", channel_id);
    println!("   (Interactive message receiving not yet implemented)");
    println!("   Press Ctrl+C to stop.");
    
    // TODO: Implement message receiving loop
    // This requires:
    // 1. Network layer integration
    // 2. Message queue/channel for incoming messages
    // 3. Decryption and display
    
    tokio::signal::ctrl_c().await?;
    println!("\nStopped listening.");
    
    Ok(())
}
