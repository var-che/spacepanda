/*
    Message - defines DHT message types.

    Responsibilities:
    `message.rs` defines the DHT message types used in the Kademlia protocol.
    It is aware of the following message types:

    Request messages:
    - FIND_NODE(target_id)
    - FIND_VALUE(key)
    - STORE_VALUE(key, value)
    - PING

    Response messages:
    - NODES(list of closest nodes)
    - VALUE(value)
    - PONG

    Serialization is done with CBOR or bincode.

    Inputs:
    - outbound/inbound network traffic

    outputs:
    - structured message enums
*/