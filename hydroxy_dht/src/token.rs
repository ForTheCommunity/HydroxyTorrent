use std::{
    net::Ipv4Addr,
    time::{Duration, Instant},
};

use rand::Rng;
use sha1::{Digest, Sha1};

const TOKEN_ROTATE_INTERVAL: Duration = Duration::from_mins(10);

pub struct TokenManager {
    current_secret: [u8; 16],
    previous_secret: Option<[u8; 16]>,
    last_rotation: Instant,
}

impl TokenManager {
    /// Get a new instance of Token Manager,.
    pub fn new() -> Self {
        Self {
            current_secret: Self::generate_new_secret(),
            previous_secret: None,
            last_rotation: Instant::now(),
        }
    }

    // generates a new random secret
    fn generate_new_secret() -> [u8; 16] {
        let mut secret = [0u8; 16];
        rand::rng().fill_bytes(&mut secret);
        secret
    }

    /// Generate an opaque token for querying IP Adresses.
    pub fn generate_token(&self, ip: &Ipv4Addr) -> Vec<u8> {
        Self::hash_token(&self.current_secret, ip)
    }

    /// Validate that a received token was issued to this IP Address
    /// (either in current or previous window).
    pub fn validate_token(&self, ip: &Ipv4Addr, token: &[u8]) -> bool {
        if token == Self::hash_token(&self.current_secret, ip) {
            return true;
        }

        if let Some(prev) = &self.previous_secret {
            if token == Self::hash_token(prev, ip) {
                return true;
            }
        }
        false
    }

    /// Rotate Secret if the rotation interval has elapsed.
    pub fn maybe_rotate(&mut self) -> bool {
        if self.last_rotation.elapsed() >= TOKEN_ROTATE_INTERVAL {
            let new_secret = Self::generate_new_secret();
            self.previous_secret = Some(self.current_secret);
            self.current_secret = new_secret;
            self.last_rotation = Instant::now();
            true
        } else {
            false
        }
    }

    fn hash_token(secret: &[u8; 16], ip: &Ipv4Addr) -> Vec<u8> {
        let mut hasher = Sha1::new();
        hasher.update(secret);
        hasher.update(ip.octets());
        let digest = hasher.finalize();
        digest[..8].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_validation() {
        let mut tm = TokenManager::new();
        let ip1 = Ipv4Addr::new(192, 168, 1, 100);
        let ip2 = Ipv4Addr::new(10, 0, 0, 1);

        let token1 = tm.generate_token(&ip1);
        let token2 = tm.generate_token(&ip2);

        assert!(tm.validate_token(&ip1, &token1));
        assert!(tm.validate_token(&ip2, &token2));
        assert!(!tm.validate_token(&ip1, &token2));

        // Force rotate
        tm.last_rotation = Instant::now() - Duration::from_secs(1000);
        assert!(tm.maybe_rotate());

        // Previous token should still be valid
        assert!(tm.validate_token(&ip1, &token1));

        // Force rotate again
        tm.last_rotation = Instant::now() - Duration::from_secs(1000);
        assert!(tm.maybe_rotate());

        // Token from 2 rotations ago should now be expired
        assert!(!tm.validate_token(&ip1, &token1));
    }
}
