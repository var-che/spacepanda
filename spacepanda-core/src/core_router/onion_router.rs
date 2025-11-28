/*
    OnionRouter - Onion building, relaying, mixing.

    Construct, send, and reply to onion-wrapped packets. Manage circuits and perform forwarding.

    Workflow for Sending (build path)
    1. Path selection - ask route_table for k relays: R1,R2,R3
    2. Ephemeral keys: generate ephemeral X25519 keypair e
    3. Use layered ephemeral per hop
    4. Layer encryption: start with final payload "P", create inner blob `L3 = AEAD(K3,header3 || P)`, then
         `L2 = AEAD(K2,header2 || L3)`, then `L1 = AEAD(K1,header1 || L2)`
    5. Send L1 to R1 over session_manager

    Workflow for Relaying (at R1)

    1. Receive "L1" from session_manager, compute "K1" using "eph_pub" + "R1_priv"
    2. Decrypt L1 using K1, parse header1 to get next hop
    3. Forward "L2" to next hop (by dialing / using session_manager)
    4. If "deliver_local" flag is set, hand the final decryipted "InnerEnvelope" to RouterHandler for dispatch

    Inputs:
      - OnionCommand::Send(dest_node, payload)
      - OnionCommand::RelayPacket(encrypted_blob)

    Outputs:
      - OnionEvent::PacketForward(next_peer, blob)
      - OnionEvent::DeliverLocal(inner_envelope)

    Notes:
    
    Mixing: optionally batch multiple decrypted inner blobs into a short window (50-200ms), shuffle,
    and forward to reduce timing correlation. This is heavier but increases anonymity.
*/