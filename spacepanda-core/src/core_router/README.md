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

Second, Onion hop crypto. Purpose is to encrypt onion layers and next hop info so each relay only learns its incomming and outgoing neighbour. It does not know the origin and the destination of the message.

```
       X   --------->    Y     ---------> Z
layer(layer(msg))
                     layer(msg)
                                          msg
```

Per layer, you include an ephemeral public key so relay can compute shared secret, and then AEAD-encrypt (header + inner onion).

Third, application payload encryption. MLS and CRDT payloads are already encrypted at the application layer. Route does not need to see message content. Router treats inner payload as opaque bytes.
