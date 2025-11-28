/*
  SessionManager

  Upgrades raw connections into authenticated sessions using Noise or equivalent,
  producing the peer_id (public key) associated with the session.

  Workflow:
  1. On Connected(conn_id): start Noise handshake (XX or IK if we know peer)
  2. When handshake succedes: derive AEAD keys and create Session object.
  3. For sending: encrypt plaintext into AEAD chhipertext => hand to transport_manager.rs as Send.
  4. For receiving: decrypt AEAD chipertext and emit PlaintextFrame(peer_id, bytes) to routing core.

  Inputs:
    - TransportEvent::Data(conn_id, bytes) (handshake frames/incoming AEAD frames)
    - TransportEvent::Connected(conn_id, remote_addr)
    - TransportEvent::Disconnected(conn_id)

  Outputs:
    - SessionEvent::PlaintextFrame(peer_id, bytes) when a full decrypting and routing.
    - SessionEvent::Established(peer_id, conn_id) for routing table.
    - SessionEvent::Closed(peer_id) when session is closed.

  Notes:
  Verify the static identity key during Noise handshake, or require signed certt post-handshake.
  Keep replay window counters.
*/