/*
    OverlayDiscovery - maintain overlay candidates

    Keep the pool of available relays fresh. Periodic process that does peer exchange and checks liveness.

    Workflow:
    1. Periodic tick:
      - call some peers with "peer_exchange" RPC to get new candidates
      - validate returned peer descriptors 
      - ping the ones we dont have currently
      - if under capacity, try bootstrap list
    2. Feed new peers into route_table.rs and notify onion_router.rs for path selection updates

    Inputs:
      - Config: desired relay pool size N,
      - Events:
        -  PeerDiscovered(peer_info)
        -  PeerLivenessResult(peer_id, is_alive)
    Outputs:
      - Events:
        -  RelayPoolUpdated(new_relay_list)

*/