/*
    RouteTable - peer metadata and selection

    Local cache of known peers, their addresses, capabilities, and stats like last-seen, latency, reliability.
    Used for path selection (onion routing) and peer dialing and DHT lookups.

    Workflow:
    1. Insert peers discovered from bootstrap, RPC peer exchange, DHT responses
    2. Maintain heath metrics (last seen, latency, failures)
    3. Provide helper: pick_diverse_relays(k) for onion path building (favour different ASNs, geos, low latency)

    Inputs:
      - RouteTableCommand::InsertPeer(peer_info)
      - RouteTableCommand::UpdatePeerStats(peer_id, stats)
      - RouteTableCommand::PickDiverseRelays(k)

    Outputs:
      - Queries like "get_best_route_for(peer_id)" or "pick_diverse_relays(k)"
    
    Notes:

    PeerInfo structure should include:

    ```rust
    struct PeerInfo {
        peer_id: PeerId,
        addresses: Vec<Multiaddr>,
        capabilities: Vec<Capability>,
        overlay_pubkey: [u8; 32],
        last_seen: DateTime<Utc>,
        latency: Option<Duration>,
        failure_count: u32,
        asn: Option<u32>,
        geo_location: Option<GeoLocation>,
    }
    ```

*/