use std::sync::Mutex;

use sha2::{Digest, Sha256};

const LIFETIME_SECONDS: u64 = 300;
const MAX_FAILED_ATTEMPTS: u8 = 3;

pub fn generate_pairing_code() -> String {
    format!("{:06}", uuid::Uuid::new_v4().as_u128() % 1_000_000)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingError {
    Invalid,
    TooManyAttempts,
    Expired,
    AlreadyUsed,
}

struct PairingState {
    hash: [u8; 32],
    issued_at: u64,
    failures: u8,
    used: bool,
    reusable: bool,
}

pub struct PairingManager {
    state: Mutex<PairingState>,
}

impl PairingManager {
    pub fn new(code: &str, issued_at: u64) -> Self {
        Self {
            state: Mutex::new(PairingState {
                hash: Sha256::digest(code.as_bytes()).into(),
                issued_at,
                failures: 0,
                used: false,
                reusable: false,
            }),
        }
    }

    #[doc(hidden)]
    pub fn reusable_for_tests(code: &str, issued_at: u64) -> Self {
        let manager = Self::new(code, issued_at);
        manager
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reusable = true;
        manager
    }

    pub fn verify(&self, code: &str, now: u64) -> Result<(), PairingError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.used && !state.reusable {
            return Err(PairingError::AlreadyUsed);
        }
        if now.saturating_sub(state.issued_at) > LIFETIME_SECONDS {
            return Err(PairingError::Expired);
        }
        if state.failures >= MAX_FAILED_ATTEMPTS {
            return Err(PairingError::TooManyAttempts);
        }
        let candidate: [u8; 32] = Sha256::digest(code.as_bytes()).into();
        let different = state
            .hash
            .iter()
            .zip(candidate)
            .fold(0_u8, |diff, (left, right)| diff | (left ^ right));
        if different != 0 {
            state.failures = state.failures.saturating_add(1);
            return if state.failures >= MAX_FAILED_ATTEMPTS {
                Err(PairingError::TooManyAttempts)
            } else {
                Err(PairingError::Invalid)
            };
        }
        state.used = !state.reusable;
        Ok(())
    }
}
