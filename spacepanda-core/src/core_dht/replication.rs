/*
    Replication - periodic replication and republishing of DHT key-value pairs

    Responsibilities:
    `replication.rs` implements the periodic replication and republishing of DHT key-value pairs.
    It handles: 
    - publishes original values you PUT
    - refresh keys
    - pushes replicas to nearest nodes
    - ensures redundancy level K
    - gargabe collection of expired keys

    Inputs:
    - timer events
    - storage list of stored keys

    outputs:
    - PUT RPC messages
    - storage updates
    - replication logs
    
*/