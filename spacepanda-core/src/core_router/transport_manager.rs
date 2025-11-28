/*
  TransportManager

  Abstracts the OS sockets and handles dialing/listening and reconnects. Provides stream of
  raw frames for the session_manager.rs to handle.

  Workflow:
  Listens on configured addresses (TCP/UDP/QUIC) and spawns tasks to accept incoming connections.
  When dialing, it establishes a socket and returns conn_id.
  

  Handles NAT traversal helpers (STUN/TURN) if configured.

  Inputs:
  are the following commands:
    - Dial(addr) -> attempts to connect to addr, emits Connected event on success
    - Listen(addr) -> starts listening on addr for incoming connections
    - Send(conn_id, bytes) -> sends bytes on the specified connection
    - Close(conn_id) -> closes the specified connection

  Outputs:
    Emits `TransportEvent::Connected(conn_id, remote_addr)` when a new connection is established.
    Emits `TransportEvent::Data(conn_id, bytes)` when data is received on a connection.
    Emits `TransportEvent::Disconnected(conn_id)` when a connection is closed.

  Important:
  Always perform basic framing (prefix length) on the bytes sent/received to avoid message boundary issues.
  Keep this module ignorant of identities; its only bytes and addresses.
  
*/