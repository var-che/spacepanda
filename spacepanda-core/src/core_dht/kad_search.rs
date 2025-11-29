/*
    KadSearch - implements interactive Kademlia search operations.

    Responsibilities:
    `kad_search.rs` implements the Kademlia search procedures for finding nodes and values in the DHT.
    Its behaviors include: iterative FIND_NODE, iterative GET_VALUE, alpha parallel lookups, termination criteria, integrate results form peers,
    detect stalled peers, ranking peers by closeness. 

    Inputs:
    - search request (key or node id)
    - routing table initial candidates
    - RPC responses from peers

    Outputs:
    - final value or final closest nodes
    - search logs
    - errors
*/