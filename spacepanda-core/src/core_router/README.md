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
