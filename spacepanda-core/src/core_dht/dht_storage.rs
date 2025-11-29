/*
    DHTStorage - local storage engine for DHT key-value pairs

    Responsibilities:
    `dht_storage.rs` implements a local storage engine for DHT key-value pairs.
    This is not a database and its very simple key-value store.
    It handles:
    - persistent map or sled/rocksdb store
    - returns stored value
    - maintains expiration
    - handles conflict resolution
    - used by replication and GET handlers

    The storage must also keep which peers store replicas.

    Inputs:
    - requests: store(key, value), get(key), delete(key)
    - load value(key)
    - refresh key (extend expiration)

    Outputs:
    - stored values
    - deletion notifications
    - expiration scans
*/