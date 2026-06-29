#![no_std]

//! # KYC Attestation Contract
//!
//! On-chain KYC attestation for AfriPay. Stores a SHA-256 hash of the user's
//! KYC data — never raw PII. Any Stellar ecosystem participant can call
//! [`is_verified`] to check a wallet's KYC status without trusting AfriPay's
//! centralized database.
//!
//! ## Access control
//! - `attest` and `revoke` — admin only
//! - `is_verified`         — public

use soroban_sdk::{contract, contractimpl, contracttype, bytes, Address, Bytes, Env, Symbol};

mod test;

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Attestation(Address),
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// On-chain KYC attestation record.
#[derive(Clone)]
#[contracttype]
pub struct Attestation {
    /// SHA-256 hash of the off-chain KYC document bundle (hex-encoded bytes).
    /// Raw PII is never stored on-chain.
    pub kyc_hash: Bytes,
    /// Unix timestamp when the attestation was issued.
    pub attested_at: u64,
    /// Unix timestamp when the attestation was revoked, or 0 if still active.
    pub revoked_at: u64,
    /// Unix ledger timestamp after which the attestation is considered expired.
    /// 0 means the attestation never expires.
    pub expires_at: u64,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct KycAttestationContract;

#[contractimpl]
impl KycAttestationContract {
    /// Initialise the contract. Must be called once.
    ///
    /// # Arguments
    /// * `admin` — The AfriPay admin address authorised to attest and revoke.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Record a KYC attestation for `user`.
    ///
    /// Only the admin may call this. Panics if the user already has an active
    /// (non-revoked) attestation.
    ///
    /// # Arguments
    /// * `admin`      — Must match the admin set during `initialize`.
    /// * `user`       — Stellar address of the verified user.
    /// * `kyc_hash`   — SHA-256 hash of the KYC document bundle. Never raw PII.
    /// * `expires_at` — Ledger timestamp after which the attestation expires.
    ///                  Pass 0 for no expiry.
    pub fn attest(env: Env, admin: Address, user: Address, kyc_hash: Bytes, expires_at: u64) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        if kyc_hash.len() == 0 {
            panic!("kyc_hash must not be empty");
        }

        // Prevent overwriting an active attestation
        if let Some(existing) = env
            .storage()
            .persistent()
            .get::<_, Attestation>(&DataKey::Attestation(user.clone()))
        {
            if existing.revoked_at == 0 {
                panic!("user already has an active attestation");
            }
        }

        let record = Attestation {
            kyc_hash,
            attested_at: env.ledger().timestamp(),
            revoked_at: 0,
            expires_at,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Attestation(user.clone()), &record);

        env.events().publish(
            (Symbol::new(&env, "KycAttested"),),
            user,
        );
    }

    /// Revoke an existing attestation for `user`.
    ///
    /// Only the admin may call this. Panics if no active attestation exists.
    ///
    /// # Arguments
    /// * `admin` — Must match the admin set during `initialize`.
    /// * `user`  — Stellar address whose attestation should be revoked.
    pub fn revoke(env: Env, admin: Address, user: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let mut record: Attestation = env
            .storage()
            .persistent()
            .get(&DataKey::Attestation(user.clone()))
            .expect("no attestation found for user");

        if record.revoked_at != 0 {
            panic!("attestation already revoked");
        }

        record.revoked_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&DataKey::Attestation(user.clone()), &record);

        env.events().publish(
            (Symbol::new(&env, "KycRevoked"),),
            user,
        );
    }

    /// Returns `true` if `user` has a current, non-revoked, non-expired KYC attestation.
    ///
    /// Public — any caller may invoke this.
    ///
    /// # Arguments
    /// * `user` — Stellar address to check.
    pub fn is_verified(env: Env, user: Address) -> bool {
        match env
            .storage()
            .persistent()
            .get::<_, Attestation>(&DataKey::Attestation(user))
        {
            Some(record) => {
                if record.revoked_at != 0 {
                    return false;
                }
                if record.expires_at != 0 && env.ledger().timestamp() > record.expires_at {
                    return false;
                }
                true
            }
            None => false,
        }
    }

    /// Revoke attestations for multiple users atomically.
    ///
    /// Only the admin may call this. Skips users with no active attestation
    /// rather than panicking, to allow partial-valid batches.
    ///
    /// # Arguments
    /// * `admin` — Must match the admin set during `initialize`.
    /// * `users` — List of Stellar addresses to revoke.
    pub fn revoke_batch(env: Env, admin: Address, users: soroban_sdk::Vec<Address>) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);

        let now = env.ledger().timestamp();
        for user in users.iter() {
            let key = DataKey::Attestation(user.clone());
            if let Some(mut record) = env
                .storage()
                .persistent()
                .get::<_, Attestation>(&key)
            {
                if record.revoked_at == 0 {
                    record.revoked_at = now;
                    env.storage().persistent().set(&key, &record);
                    env.events().publish(
                        (Symbol::new(&env, "KycRevoked"),),
                        user,
                    );
                }
            }
        }
    }

    /// Return the full attestation record for `user`, or panic if none exists.
    ///
    /// # Arguments
    /// * `user` — Stellar address to look up.
    pub fn get_attestation(env: Env, user: Address) -> Attestation {
        env.storage()
            .persistent()
            .get(&DataKey::Attestation(user))
            .expect("no attestation found for user")
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn assert_admin(env: &Env, caller: &Address) {
        let stored: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized");
        if caller != &stored {
            panic!("unauthorized: caller is not admin");
        }
    }
}
