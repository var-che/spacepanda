/*
    DhtNode - represents a full DHT participant node.
    Coordinates routing table, replication, searching, storing, messaging.


    Responsibilities:
    `dht_node.rs` is the brain of the DHT subsystem.

    A DhtNode instance manages:
    - owns the local nodes DHT ID
    - holds the DHT routing table
    - exposes operations like: put (key, value), get(key), find_node(node_id), replicate()
    - communicates with the router layer to send RPC DHT messages
    - listens to incomming DHT RPC messages
    - updates routing table
    - ensures DHT config (replication factor, alpha concurrency, timeouts, etc) is respected
    - performs periodic maintenance tasks (replication, refreshing buckets, etc)

    Inputs:
    - request from the upper application: GetValue(key), PutValue(key, value), FindNode(node_id)
    - DHT messages from remote peers
    - timer events for refresh/replication
    - router events (peer discovered, peer disconnected, etc)

    Outputs:
    - RPC requests to remote peers via the router layer
    - updates in routing table
    - responses to application-level API calls
    - events (VALUE_FOUND, VALUE_STORED,SEARCH_FAILED, etc )
    - logs for monitoring and debugging

*/