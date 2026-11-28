use std::{
    net::IpAddr,
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
    pub fn new() -> Self {
        Self {
            current_secret: Self::generate_new_secret(),
            previous_secret: None,
            last_rotation: Instant::now(),
        }
    }

    fn generate_new_secret() -> [u8; 16] {
        let mut secret = [0u8; 16];
        rand::rng().fill_bytes(&mut secret);
        secret
    }

    /// Generate an opaque token bound to the querying IPv4 or IPv6 address.
    pub fn generate_token(&self, ip: &IpAddr) -> Vec<u8> {
        Self::hash_token(&self.current_secret, ip)
    }

    /// Validate that a received token was issued to this IP address
    /// (either in the current or previous rotation window).
    pub fn validate_token(&self, ip: &IpAddr, token: &[u8]) -> bool {
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

    /// Rotate secret if the rotation interval has elapsed.
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

    fn hash_token(secret: &[u8; 16], ip: &IpAddr) -> Vec<u8> {
        let mut hasher = Sha1::new();
        hasher.update(secret);
        match ip {
            IpAddr::V4(v4) => hasher.update(v4.octets()),
            IpAddr::V6(v6) => hasher.update(v6.octets()),
        }
        let digest = hasher.finalize();
        digest[..8].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_token_generation_and_validation_dual_stack() {
        let mut tm = TokenManager::new();
        let ip_v4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let ip_v6 = IpAddr::V6("2406:9cc0:200:1521::1".parse::<Ipv6Addr>().unwrap());

        let token4 = tm.generate_token(&ip_v4);
        let token6 = tm.generate_token(&ip_v6);

        assert!(tm.validate_token(&ip_v4, &token4));
        assert!(tm.validate_token(&ip_v6, &token6));
        assert!(!tm.validate_token(&ip_v4, &token6));
        assert!(!tm.validate_token(&ip_v6, &token4));

        // Force rotate
        tm.last_rotation = Instant::now() - Duration::from_secs(1000);
        assert!(tm.maybe_rotate());

        // Previous window token should still validate
        assert!(tm.validate_token(&ip_v4, &token4));
        assert!(tm.validate_token(&ip_v6, &token6));

        // Second rotation expires the old token
        tm.last_rotation = Instant::now() - Duration::from_secs(1000);
        assert!(tm.maybe_rotate());
        assert!(!tm.validate_token(&ip_v4, &token4));
        assert!(!tm.validate_token(&ip_v6, &token6));
    }
}
