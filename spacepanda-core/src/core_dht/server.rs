/*
    Server - Handles inbound DHT RPC requests.

    Responsibilities:
    `server.rs` handles inbound DHT RPC requests.
    It is equivalent of DHT RPC listener.

    Workflow:
    - receive message from router
    - decode DHT message
    - update routing table
    - call handler:
      - on_find_node
      - on_find_value
      - on_store_value
      - on_ping
    - respond through router

    Inputs:
    - inbound DHT RPC messages from router
    - router callback

    Outputs:
    - DHTReesponse messages back to requester
    - storage operations
    - routing table mutations
*/