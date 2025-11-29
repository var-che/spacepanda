/*
   RouterHandle

   Single enty point for the rest of the application to interact with the router.
   It exposes concise async methods for starting/stopping the router, sending/receiving messages,
   and managing connections.

   E.G: send_direct(peer_id, bytes) -> Result<()>
        send_anonymous(peer_id, bytes) -> Result<()>
        broadcast(topic, bytes) -> Result<()>
        subscribe(topic) -> Stream<InnerEnvelope>
        rpc_call(peer_id, method, params) -> Result<response_bytes>

    Workflow and where it sits?

    It is called by the APP (MLS layer), to send encrypted payloads to other peers.
    Internally it packages a RouterCommand and pushes it onto the router actor main channel.
    And it awaits responses for RPC (correlated by request ID).

    Example: `send_anonymous(peer_id, mls_cyphertext)` will create a
     `RouterCommand::OverlaySend { dest:peer_id, payload }`

*/