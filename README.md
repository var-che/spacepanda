TODO soon

## Security Documentation

SpacePanda implements comprehensive security measures based on modern cryptographic protocols:

- **[Threat Model](docs/threat-model.md)** - STRIDE analysis, attack trees, security controls
- **[Privacy Audit](docs/privacy-audit.md)** - Complete privacy data flow analysis
- **[Security Quick Reference](docs/security-quick-reference.md)** - Developer checklist and security properties
- **[Phase 3 Security Audit](docs/phase3-security-audit.md)** - Comprehensive testing and hardening results

**Key Security Features**:

- End-to-end encryption via MLS protocol (RFC 9420)
- Metadata encryption with ChaCha20-Poly1305 AEAD
- HKDF-based key derivation with domain separation
- 1273 security tests, 0 known vulnerabilities in dependencies
- Strong privacy posture (no user tracking, sealed sender, minimal metadata)

## Bootstrap

There should be one keypair generated, and it is used globally.

- [] Identity Keypairs
- - [+] Generate global identity keypair
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

nix develop --command cargo test --package spacepanda-core --lib -- core_identity::global::tests::test_create_global_identity --exact --nocapture
