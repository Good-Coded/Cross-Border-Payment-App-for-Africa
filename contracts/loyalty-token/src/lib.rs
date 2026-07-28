#![no_std]

//! # AfriPay Loyalty Token — SEP-41 Compatible Fungible Token
//!
//! Issues loyalty points to users for each transaction and allows redemption
//! for fee discounts.
//!
//! ## Earn rate
//! 1 loyalty point per 1 XLM (or XLM-equivalent) of transaction volume.
//! The backend calls [`mint`] after each successful payment.
//!
//! ## Redemption
//! 100 points → 50 % fee discount on the next transaction.
//! The backend calls [`redeem`] before a payment to burn 100 points and
//! record the discount entitlement on-chain.
//!
//! ## SEP-41 interface
//! Implements the full SEP-41 token interface:
//! `allowance`, `approve`, `balance`, `burn`, `burn_from`,
//! `decimals`, `mint`, `name`, `symbol`, `total_supply`,
//! `transfer`, `transfer_from`.

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String,
};

#[contracttype]
pub struct AllowanceValue {
    pub amount: i128,
    pub expires_at: u64,
}

mod test;

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    TotalSupply,
    MaxSupply,
    TransferFeeBps,
    Balance(Address),
    Allowance(Address, Address), // (owner, spender)
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Points required to earn a 50 % fee discount.
const REDEMPTION_THRESHOLD: i128 = 100;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct LoyaltyTokenContract;

#[contractimpl]
impl LoyaltyTokenContract {
    // ── Admin ─────────────────────────────────────────────────────────────────

    /// Initialise the contract. Must be called once before any other function.
    ///
    /// # Arguments
    /// * `admin`            — Address authorised to mint tokens (the AfriPay backend).
    /// * `max_supply`       — Hard ceiling on total points that can ever be minted (must be > 0).
    /// * `transfer_fee_bps` — Fee taken on peer transfers in basis points (0–10000). Fee is
    ///                        credited to the admin. Pass 0 to disable fees.
    pub fn initialize(env: Env, admin: Address, max_supply: i128, transfer_fee_bps: u32) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        if max_supply <= 0 {
            panic!("max_supply must be positive");
        }
        if transfer_fee_bps > 10000 {
            panic!("transfer_fee_bps must be <= 10000");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::TotalSupply, &0i128);
        env.storage().persistent().set(&DataKey::MaxSupply, &max_supply);
        env.storage().persistent().set(&DataKey::TransferFeeBps, &transfer_fee_bps);
    }

    // ── SEP-41: token metadata ────────────────────────────────────────────────

    pub fn name(env: Env) -> String {
        String::from_str(&env, "AfriPay Loyalty Points")
    }

    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "ALP")
    }

    /// Loyalty points have no sub-unit — decimals = 0.
    pub fn decimals(_env: Env) -> u32 {
        0
    }

    // ── SEP-41: supply & balances ─────────────────────────────────────────────

    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    pub fn max_supply(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxSupply)
            .expect("not initialized")
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(account))
            .unwrap_or(0)
    }

    // ── SEP-41: allowances ────────────────────────────────────────────────────

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        let entry: Option<AllowanceValue> = env
            .storage()
            .persistent()
            .get(&DataKey::Allowance(owner, spender));
        match entry {
            None => 0,
            Some(v) => {
                if env.ledger().timestamp() > v.expires_at {
                    0
                } else {
                    v.amount
                }
            }
        }
    }

    /// Approve `spender` to transfer up to `amount` points on behalf of the
    /// caller until `expires_at` (inclusive, Unix ledger timestamp).
    ///
    /// Set `amount` to 0 to revoke an existing allowance.
    ///
    /// # Race-condition guard
    /// To prevent the ERC-20 double-spend race condition, this function panics
    /// if `amount > 0` and a non-zero allowance already exists.  Callers must
    /// first call `approve(…, 0, …)` to reset the allowance before setting a
    /// new non-zero value.  Alternatively, use [`increase_allowance`] /
    /// [`decrease_allowance`] which are inherently race-condition free.
    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128, expires_at: u64) {
        if amount < 0 {
            panic!("amount must be non-negative");
        }
        if expires_at < env.ledger().timestamp() {
            panic!("expires_at must be in the future");
        }
        // Race-condition guard: prevent double-spend by rejecting a non-zero
        // approval while a non-zero allowance is still active.
        if amount > 0 {
            let current = Self::allowance(env.clone(), owner.clone(), spender.clone());
            if current > 0 {
                panic!("Reset to zero before setting new allowance");
            }
        }
        owner.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner, spender), &AllowanceValue { amount, expires_at });
    }

    /// Increment the existing allowance for `spender` by `delta`.
    ///
    /// If no allowance exists, one is created with `expires_at` set to the
    /// maximum representable ledger timestamp (effectively never expires).
    /// If an allowance already exists its `expires_at` is preserved.
    ///
    /// Emits an `AllowanceChanged` event with `(owner, spender, old_value,
    /// new_value)`.
    ///
    /// Requires authorization from `owner`.
    pub fn increase_allowance(env: Env, owner: Address, spender: Address, delta: i128) {
        if delta <= 0 {
            panic!("delta must be positive");
        }
        owner.require_auth();

        let entry: Option<AllowanceValue> = env
            .storage()
            .persistent()
            .get(&DataKey::Allowance(owner.clone(), spender.clone()));

        let (old_amount, expires_at) = match entry {
            None => (0i128, u64::MAX),
            Some(v) => {
                if env.ledger().timestamp() > v.expires_at {
                    (0i128, u64::MAX)
                } else {
                    (v.amount, v.expires_at)
                }
            }
        };

        let new_amount = old_amount + delta;

        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner.clone(), spender.clone()), &AllowanceValue {
                amount: new_amount,
                expires_at,
            });

        env.events().publish(
            (symbol_short!("allowance"), symbol_short!("changed"), owner, spender),
            (old_amount, new_amount),
        );
    }

    /// Decrement the existing allowance for `spender` by `delta`.
    ///
    /// Panics if `delta` exceeds the current allowance (underflow protection).
    ///
    /// Emits an `AllowanceChanged` event with `(owner, spender, old_value,
    /// new_value)`.
    ///
    /// Requires authorization from `owner`.
    pub fn decrease_allowance(env: Env, owner: Address, spender: Address, delta: i128) {
        if delta <= 0 {
            panic!("delta must be positive");
        }
        owner.require_auth();

        let entry: Option<AllowanceValue> = env
            .storage()
            .persistent()
            .get(&DataKey::Allowance(owner.clone(), spender.clone()));

        let (old_amount, expires_at) = match entry {
            None => (0i128, u64::MAX),
            Some(v) => {
                if env.ledger().timestamp() > v.expires_at {
                    (0i128, u64::MAX)
                } else {
                    (v.amount, v.expires_at)
                }
            }
        };

        if delta > old_amount {
            panic!("delta exceeds current allowance");
        }

        let new_amount = old_amount - delta;

        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner.clone(), spender.clone()), &AllowanceValue {
                amount: new_amount,
                expires_at,
            });

        env.events().publish(
            (symbol_short!("allowance"), symbol_short!("changed"), owner, spender),
            (old_amount, new_amount),
        );
    }

    // ── SEP-41: transfers ─────────────────────────────────────────────────────

    /// Transfer `amount` points from the caller to `to`.
    /// If a `transfer_fee_bps` was set at initialisation, the fee is deducted
    /// from the transferred amount and credited to the admin.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        from.require_auth();
        Self::_debit(&env, &from, amount);

        let fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::TransferFeeBps)
            .unwrap_or(0);
        let fee = if fee_bps > 0 { amount * fee_bps as i128 / 10000 } else { 0 };
        let net = amount - fee;

        Self::_credit(&env, &to, net);
        if fee > 0 {
            let admin: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Admin)
                .expect("not initialized");
            Self::_credit(&env, &admin, fee);
        }
    }

    /// Transfer `amount` points from `from` to `to` using an allowance.
    /// The fee (if any) is deducted from the transferred amount and credited to admin.
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        spender.require_auth();

        let entry: AllowanceValue = env
            .storage()
            .persistent()
            .get(&DataKey::Allowance(from.clone(), spender.clone()))
            .unwrap_or(AllowanceValue { amount: 0, expires_at: 0 });

        if env.ledger().timestamp() > entry.expires_at {
            panic!("allowance expired");
        }
        if entry.amount < amount {
            panic!("insufficient allowance");
        }

        env.storage()
            .persistent()
            .set(&DataKey::Allowance(from.clone(), spender), &AllowanceValue {
                amount: entry.amount - amount,
                expires_at: entry.expires_at,
            });

        Self::_debit(&env, &from, amount);

        let fee_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::TransferFeeBps)
            .unwrap_or(0);
        let fee = if fee_bps > 0 { amount * fee_bps as i128 / 10000 } else { 0 };
        let net = amount - fee;

        Self::_credit(&env, &to, net);
        if fee > 0 {
            let admin: Address = env
                .storage()
                .persistent()
                .get(&DataKey::Admin)
                .expect("not initialized");
            Self::_credit(&env, &admin, fee);
        }
    }

    // ── SEP-41: burn ──────────────────────────────────────────────────────────

    /// Burn `amount` points from the caller's balance.
    pub fn burn(env: Env, from: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        from.require_auth();
        Self::_debit(&env, &from, amount);
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply, &(supply - amount));
    }

    /// Burn `amount` points from `from` using an allowance granted to the
    /// caller.
    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        spender.require_auth();

        let entry: AllowanceValue = env
            .storage()
            .persistent()
            .get(&DataKey::Allowance(from.clone(), spender.clone()))
            .unwrap_or(AllowanceValue { amount: 0, expires_at: 0 });

        if env.ledger().timestamp() > entry.expires_at {
            panic!("allowance expired");
        }
        if entry.amount < amount {
            panic!("insufficient allowance");
        }

        env.storage()
            .persistent()
            .set(&DataKey::Allowance(from.clone(), spender), &AllowanceValue {
                amount: entry.amount - amount,
                expires_at: entry.expires_at,
            });

        Self::_debit(&env, &from, amount);
        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply, &(supply - amount));
    }

    // ── Loyalty-specific ──────────────────────────────────────────────────────

    /// Mint `amount` loyalty points to `to`.
    ///
    /// Only the admin (AfriPay backend) may call this.
    /// Called after each successful payment: 1 point per 1 XLM of volume.
    ///
    /// # Arguments
    /// * `admin`  — Must match the admin set during `initialize`.
    /// * `to`     — Recipient wallet address.
    /// * `amount` — Points to mint (must be > 0).
    pub fn mint(env: Env, admin: Address, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized");

        if admin != stored_admin {
            panic!("unauthorized: caller is not admin");
        }

        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        let cap: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxSupply)
            .expect("not initialized");
        if supply + amount > cap {
            panic!("minting would exceed max supply");
        }

        Self::_credit(&env, &to, amount);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply, &(supply + amount));
    }

    /// Redeem 100 loyalty points for a 50 % fee discount on the next
    /// transaction.
    ///
    /// Burns exactly `REDEMPTION_THRESHOLD` (100) points from the caller's
    /// balance. Returns `true` if the redemption succeeded.
    ///
    /// The backend checks the return value and applies the discount before
    /// broadcasting the next payment.
    ///
    /// # Arguments
    /// * `account` — The user redeeming points; must authorise this call.
    pub fn redeem(env: Env, account: Address) -> bool {
        account.require_auth();

        let bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(account.clone()))
            .unwrap_or(0);

        if bal < REDEMPTION_THRESHOLD {
            return false;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Balance(account), &(bal - REDEMPTION_THRESHOLD));

        let supply: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::TotalSupply, &(supply - REDEMPTION_THRESHOLD));

        true
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn _credit(env: &Env, to: &Address, amount: i128) {
        let bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
    }

    fn _debit(env: &Env, from: &Address, amount: i128) {
        let bal: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(bal - amount));
    }
}
