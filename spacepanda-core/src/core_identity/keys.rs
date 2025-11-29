/*
    Keys in this module are pseudonymous keys, and not MLS keys.
    They are public-facing, used for "sender label" inside of the channel
    not cryptographically tied to MLS membership
    can rotate independantly
    and are only relevant to identity layer

    What are responsibilities of this module?
    Generate a pseudonymous keypair for a channel
    Map `channel_hash` -> pseudonymous keypair
    Regenerate keypair when user wants a new identity for a channel
    Provide function to fetch the keypair for a channel.
*/

pub struct Keypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl Keypair {
    pub fn generate() -> Self {
        // TODO: Implement actual Ed25519 key generation
        // For now, using placeholder values
        let public_key = vec![0; 32];
        let private_key = vec![0; 64];
        Keypair { public_key, private_key }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = Keypair::generate();
        assert_eq!(keypair.public_key.len(), 32);
        assert_eq!(keypair.private_key.len(), 64);
    }
}
