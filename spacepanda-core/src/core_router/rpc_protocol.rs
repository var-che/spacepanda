/*
    RpcProtocol - framed RPC and method dispatch

    Standardize controll messages (peer exchange, DHT requests, ping, snapshot fetches)
    Provide request/response semantics with timeouts and retry.

    Workflow:

    1. Expose rpc_call(peer_id, method, params) which:
        - signs or attaches an auth signature
        - packages a `RpcRequest {id, method params}` 
        - ask routing core to send (direct or anonymous)
        - waits on local response map keyed by id for a RpcResponse or timeout

    2. On receive:
        - parse frame to RpcRequest or RpcResponse
        - dispatch to appropriate handler (DHT handler, peer exchange, etc)
        - send RpcResponse back

    Inputs:
        - RpcCommand::RpcCall(peer_id, method, params)
        - Incomminng PlaintextFrame(peer_id, bytes) from session_manager

    Outputs:
        - Synchronous results to callers (awaited futures)
        - Events to other modules (e.g. DHT handler)

    Notes:

    Message sturcture for example, suing serde_cbor:
    ```json
    {
        "type": "request",
        "id": "unique_request_id",
        "method": "get_peer_info",
        "params": {
            "some_param": "<hex>"
        }
    }
    ```
*/