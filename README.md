TODO soon

## Bootstrap

There should be one keypair generated, and it is used globally.

- [] Identity Keypairs
- - [] Generate global identity keypair
- - [] Implement per channel pseudononymus keypair logic
- - [] Store identities in a local keystore
- - [] Serialization/deserialization of keys
- [] Identity API
- - [] create_identity()
- - [] load_identity()
- - [] get_identity_for_channel(channel_id)

Later on, user on one device will be able to create multiple identities on the same machine. It will let the user to create pseudonyms, throwaway identities, per-channel unlinkability.

# Enter Nix development environment

nix develop

# Run all tests

cargo test

# Run the CLI

cargo run --bin spacepanda test "Hello World"

# Run the logging example

cargo run --example logging_demo
