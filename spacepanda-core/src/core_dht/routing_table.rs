/*
    DHTTable DHT version -  Kademlia bucket structure, store nearest peers by XOR distance

    Responsibilities:
    `routing_table.rs` implements the Kademlia routing table structure.
    It performs: bucket maintenance, insert/remove peer, replace stale entries, select clsoest nodes, respond to node lookup queries, refresh random IDs.

    Inputs:
    - peer info discovered
    - succesfull/failed RPC calls
    - DHT messages from peers

    Outputs:
    - list of closest K peers for a given key
    - bucket refresh events
*/