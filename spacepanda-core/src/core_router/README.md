The purpose of the `core_router` module is to take a higl-level app interactions, like:

send this MLS encrypted message to the peer;
publish this CRDT op to the channel;
fetch this snapshot from the DHT;

and reliably deliver those bytes across privacy-aware P2P overlay.

```
APP (crdt, mls) Initiates a peer interaction command. "Send this msg to Bob"
"Send this encrypted MLS to Bob"
"Fetch snapshot from DHT"
↓
/router module (Routes the command to the appropriate peer)
↓
Interacting with peers (The final stage where the actual peer-to-peer interaction occurs.)
```

Lets describe more the responsabilities of this module:

It should choose how to deliver the message: direct session or anonimous overlay;
It should build and manage paths - onion;
Manage sessions - authenticated, encrypted connections;
Drives the transport -QUIC/TCP/WebRTC
Implements RPC, DHT, Pubsub primitives on top of transport
Receives inbound frames, verifies/unwraps them, and dispatches them to the app modules (MLS,CRDT)
Provides reliability, retries, backoffs, caching, and diagnostics.

High level architecture of the components in this module

```
    APP     <--->  RouterHandle <--> RoutingCore   <-->    SessionManager <--->     TransportManager
(MLS, CRDT)             |                |                       /\                   (QUIC/TCP)
                        |                |                       |
                        |                |_ OnionRouter -- Relay nodes (other peers)
                        |                        /\
                        |_____ DHT / Gossip _____|

```

RouterHandle - public API used by the app (send, rpc_call, fetch_snapshot, subscribe_topic)
RoutingCore - (routing.rs + route_table) - decides path, chooses relays, uses route cache
SessionManager - manages Noise handshakes, AEAD keys, session state.
TransportManager - raw sockets; dial/listen, reconnection.
OnionRouter - builds onion layers, relays packets, mixes optionally.
DHT & Gossip - use the overlay for privacy; store & lookup resources.

The `/router` module uses cryptography at two logical levels:

First, link/session layer (p2p transport auth). The purpose is to authenticate and encrypt the transport connections between two endpoints. Usually it is used Noise protocol (xx handshake) or TLS-like AEAD. The outcome produces ephemeral symetric keys, protects against the MITM, provides peer identity ( by verifying signed static keys)

```
Alice  --------- transport connection ------------  Bob
            |( authenticate and encrypt )|
```

Second, Onion hop crypto. Purpose is to encrypt onion layers and next hop info so each relay only learns its incomming and outgoing neighbour. It does not know the origin and the destination of the message. When the node unpacs the layer, it should see the next address where to forward the message to, and should do it.

```
       X   --------->    Y     ---------> Z
layer(layer(msg))
                     layer(msg)
                                          msg
```

Per layer, you include an ephemeral public key so relay can compute shared secret, and then AEAD-encrypt (header + inner onion).

Third, application payload encryption. MLS and CRDT payloads are already encrypted at the application layer. Route does not need to see message content. Router treats inner payload as opaque bytes.

### Wire formats and concrete frame layout

We need to be explicit about on-the-wire frames as we will write a code against those layouts.

1. Top level frame (sent over a session/transport)
   To note, all the frames are sent over a session that is already encrypted via Noise or QUIC-level TLS. The frame is a simple envelope you can parse quickly.

```rust
Frane {
    version: u8, // 1, 2, 3
    frame_type: u8, // ONION, RPC, DHT_REQ, DHT_RES, HEARTBEAT...
    reserved: u16, // for future flags
    length: u32, // Length of the payload
    payload: [u8],  // (LEN) opaque bytes, may be AEAD ciphertext for session
    mac: [u8; 16] // optional session layer MAC if you use one
}
```

2. Onion layer (inner format)
   Each onion-layer blob (what Relay decrypts) contains:

```rust
OnionLayer {
    ephemeral_pubkey: [u8;32], // sender ephemeral X25519 for this layer
    header_nonce: [u8,12],     //
    chipertext: bytes,         // AEAD(encrypt(header || inner))
}
```

after AEAD decrypt, the cleartext header contains:

```rust
OnionHeader {
    next_hop: Option<Address>,   // IP:port or relay token (for overlay forwarding)
    deliver_local: bool,         // true if final hop should deliver to local node
    ttl:u16,                     // decrement at each hop
    flags: u8,                   // TBD, but flags are always good to have in reserve
}
inner_payload: bytes             // either another OnionLayer or final InnerEnvelope
```

3. Final InnerEnvelope (delivered to application via Router)

```rust
InnerEnvelope {
    envelope_type: u8,                      // MLS_PAYLOAD,CRDT_OP, WELCOME, RPC, DHT...
    channel_id?(or channel_hash?): [u8,32], // optional for pubsub/mls
    epoch_id?: [u8,32],                     // optional epoch ref
    body: bytes                             // e.g: MLS cyphertext or RPC payload
}
```

Note that all inner payloads should be application-encrypted (MLS or HPKE for welcome), so router cannot read them even at the final hop (except for the intended recipient who has MLS state).
Also, non deliverable errors or malformed frames should be dropped and the offending session possibly rate-limited.

## Message lifecycle

Its easier to follow some basic steps of what happens from the APP, down to sending the message to the node. We can asume anonymous MLS message set from user code to recipient.

1. First, APP builds MLS ciphertext `mls_ct = MLS.encrypt(epoch, crdt_op_bytes)`
2. APP calls `router.send_anonymous(dest_node_id, mls_ct)`
3. RouterHandle packages into `RouterCommand::OverlaySend` and sends to actor
4. RoutingCore asks `route_table.pick_diverse_relays(k=3)` to choose R1,R2,R3 and creates an OverlayRoute.
5. OnionRouter builds onion layers:

- compute ephemeral keys and per-hop AEAD keys
- create nested AEAD ciphertexts "L1"

6. RoutingCore sends "L1" to "SessionManager" to send to "R1". If no session to "R1" exists, TransportManager dials "R1" and performs Noise handshake, and then send.
7. R1 receives L1, decrypts layer, forwards L2 to R2 via its session.
8. R2 decrypts layer, forwards L3 to R3.
9. R3 decrypts final layer, header `deliver_local = true`, instruct R3 to deliver final inner envelope to final recipient (maybe dest_node is R3 or an address reachalble from R3). R3 uses its session to deliver to the recipient or uses direct local delivery if it is final. If the final recipient is the last hop iteslf, it handles the (still-mls-encrypted) inner envelope and passes to its local RouterHandle, which passess to MLS decryptor.
10. Recipient receives the inner envelope, decrypts MLS ciphertext, applies CRDT op.

With thsese steps, no single relay knows the full path, MLS payload remains encrypted e2e, and if any relay fails, the sender can build another route and retry with exponential backoff.

TODO:
Phase 0:

- [+] Implement `transport_manager.rs` with basic TCP dial/listen
- [+] Implement `session_manager.rs` to manage sessions over transport, and and Noise handshake
- [+] Implement `rpc_protocol.rs` and do Hello, ping
- [+] Implement `router_handle.send_direct` and `rpc_call`

Phase 1:

- [+] Implement `route_table.rs` and `overlay_discovery.rs`
- [+] Implement `onion_router.rs` , build/peel with static relays but no batching
- [+] Implement `router_handle.send_anonymous` (build path, send)

Phase 2 — DHT & Network Layer

[ ] 2.1 DHT Message Types
[ ] Define Store/Get/FindNode RPCs
[ ] Add signatures
[ ] Add support in rpc_protocol.rs

[ ] 2.2 Routing Table (full implementation)
[ ] K-buckets
[ ] Splitting
[ ] LRU eviction
[ ] Refresh timers
[ ] Stale-node recovery

[ ] 2.3 DHT Query Executor
[ ] lookup_node()
[ ] lookup_value()
[ ] iterative routing
[ ] direct + anonymous send

[ ] 2.4 DHT Storage Engine
[ ] embedded database
[ ] persist key=value
[ ] replication factor
[ ] periodic re-replicate

[ ] 2.5 NAT traversal
[ ] external address discovery
[ ] hole punching
[ ] relay fallback

[ ] 2.6 DHT Test Harness
[ ] multi-node local cluster
[ ] random failures
[ ] convergence tests

---

Phase 3 — CRDT + MLS Messaging System

[ ] 3.1 CRDT foundation
[ ] OR-Map
[ ] RGA/LSEQ
[ ] LWW-Register
[ ] integrate with DHT

[ ] 3.2 MLS groups
[ ] per-channel MLS state
[ ] join/welcome
[ ] commit proposals
[ ] encrypt ops

[ ] 3.3 Message ingest pipeline
[ ] decrypt
[ ] validate
[ ] apply CRDT
[ ] persist

[ ] 3.4 Snapshotting
[ ] snapshot create
[ ] snapshot pull (rehydrate)
[ ] integrate with joins

[ ] 3.5 Identity & Roles
[ ] role crdt
[ ] role enforcement
[ ] banned-kick flow
