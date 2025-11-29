The DHT module

first, it is responsible for content addressing - everything! from channels, messasges, user profiles, crdt ops, snapshots, they are all stored as hashes as keys, no UUID.

second, distributed indexing - nodes collaboratevely maintain a lookup system for :
"Where is entry <hash> stored?"
"Which nodes are closest to <hash> in ID space?"

third, reduntant storage - each piece of data is stored on multiple nodes, to provide availability and fault tolerance.

fourth, self-healing - nodes periodically verify stored data, re-replicate lost pieces, and ensure consistency across replicas.

fifth, efficient lookup - typically O(log N) hops to find data in a network of N nodes, using iterative routing and k-buckets.

six, privacy configurable: can retreive data directly (fast), or anonymously via onion routing (private).

seven, validation layer - reject invalid or malicious data before storing it. check for bad signatures, invalid CRDT ops, identity mismatch, wrong channel ACL, expired messages, etc.

eight, resistance- local node must store: DHT records, routing table, CRDT logs, snapshouts.

Important: this subsystem sits ON TOP of /router and BELLOW of /crdt and /mls.

## Top level stack

```
               ┌───────────────────────────┐
               │        /crdt + /mls       │
               └───────────────┬───────────┘
                               │
                      (uses DHT API)
                               │
                     ┌────────▼────────┐
                     │   /core_dht     │
                     │  (store / get)  │    <- is between app and router>
                     └───────┬─────────┘
                     (DHT RPC messages)
                             │
        ┌────────────────────┴─────────────────────┐
        │                                          │
 ┌──────▼─────────┐                       ┌────────▼────────┐
 │ direct send     │                       │   onion send    │
 │ /router/session │                       │ /router/onion   │
 └──────┬──────────┘                       └────────┬────────┘
        │                                           │
        └─────────────────────┬─────────────────────┘
                              v
                   ┌──────────────────────┐
                   │   TCP / QUIC socket  │
                   └──────────────────────┘
```

## Type of DHT

Kademlia-like DHT with XOR metric, K-buckets, iterative routing, focusing on privacy and data validation.

## Complete data lifecycle Mermaid diagram

```mermaid
flowchart TD

%% =====================
%% APPLICATION LAYER
%% =====================
subgraph APP[Application Layer - CRDT MLS Channels]
    A1[app.store - publish update]
    A2[app.get - fetch state]
end

%% =====================
%% DHT LAYER
%% =====================
subgraph DHT[DHT Layer - Hash addressing replication]
    D1[dht_handle.store]
    D2[dht_handle.get]
    D3[routing_table - Kademlia buckets]
    D4[query_executor - FIND_NODE GET PUT]
    D5[local_storage]
end

%% =====================
%% ROUTING LAYER
%% =====================
subgraph ROUTER[Routing Privacy Layer - Direct and Onion]
    R1[rpc_protocol.wrap]
    R2[router_handle - choose direct or anon]
    R3[session_manager - Noise handshake]
    R4[onion_router - build peel circuits]
end

%% =====================
%% TRANSPORT LAYER
%% =====================
subgraph TRANSPORT[Transport Layer - TCP QUIC]
    T1[tcp_dial and tcp_listen]
    T2[quic_dial and quic_listen]
end

%% =====================
%% NETWORK
%% =====================
subgraph NET[Network Layer - Raw Internet]
    N1[Direct Peer]
    N2[Relay Nodes]
end


%% TOP DOWN FLOW
A1 --> D1
A2 --> D2

D1 --> D4
D2 --> D4

D4 --> D3
D4 --> D5

D4 --> R1
R1 --> R2

R2 -->|direct| R3
R2 -->|anonymous| R4

R3 --> TRANSPORT
R4 --> TRANSPORT

TRANSPORT --> NET

%% RETURN FLOW
NET --> TRANSPORT
TRANSPORT --> R3
R3 --> R1
R1 --> D4
D4 --> D5
D4 --> A2

```

## Message flow top to bottom

```
app.store()
   ↓
dht_handle.store()
   ↓
query_executor → routing_table → build dht_message
   ↓
rpc_protocol.wrap(dht_message)
   ↓
router_handle → (direct OR onion)
   ↓
session_manager.encrypt()
   ↓
transport_mgr.send_tcp_or_quic()
   ↓
Raw Internet (direct peer or relay chain)
```

## Message flow bottom to top

```
Raw Internet packet arrives
   ↓
transport_mgr.accept()
   ↓
session_manager.decrypt()
   ↓
rpc_protocol.parse()
   ↓
dht_message → query executor
   ↓
dht_handle → app layer
```
