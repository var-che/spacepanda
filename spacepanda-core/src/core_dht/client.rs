/*
    Client - sends outbound DHT RPC messages.

    Responsibilities:
    `client.rs` implements the DHT client responsible for sending outbound DHT RPC messages.
    It is a thin wrapper to send messages via router:

    ```rust
    router_handle.send_direct(peer_id, DhtMessage::FindNode{...})
    ```

    it handles: timeouts, retries, and updating routing table with response quality.

    Inputs:
    - API calls from dht_node
    - search requests (kad_search)

    outputs:
    - request to router
    - resolved responses -> search engine
*/