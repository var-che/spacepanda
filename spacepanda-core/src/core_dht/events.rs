/*
    Events - event definitions for DHT internal and external notifications

    Events:
    - ValueFound
    - SearchCompleted
    - SearcFailed
    - BucketUpdated
    - PeerExpired
    - KeyReplicated
    - ValueStored

    This is also used for logging, metrics and high level logic.

    Inputs:
    - triggered accross subsystems

    Outputs:
    - passed to listeners / subscribers
*/