#![cfg(test)]

use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

use crate::{LoyaltyTokenContract, LoyaltyTokenContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

const DEFAULT_MAX_SUPPLY: i128 = 1_000_000_000;
const BASE_TIMESTAMP: u64 = 1_000_000;
const FAR_FUTURE: u64 = 9_999_999_999;

fn setup() -> (Env, LoyaltyTokenContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = BASE_TIMESTAMP);
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &DEFAULT_MAX_SUPPLY);
    (env, client, admin)
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_initialize_sets_zero_supply() {
    let (_, client, _) = setup();
    assert_eq!(client.total_supply(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_, client, admin) = setup();
    client.initialize(&admin, &DEFAULT_MAX_SUPPLY);
}

#[test]
#[should_panic(expected = "max_supply must be positive")]
fn test_initialize_zero_max_supply_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin, &0);
}

#[test]
fn test_initialize_stores_max_supply() {
    let (_, client, _) = setup();
    assert_eq!(client.max_supply(), DEFAULT_MAX_SUPPLY);
}

// ── metadata ──────────────────────────────────────────────────────────────────

#[test]
fn test_name() {
    let (env, client, _) = setup();
    assert_eq!(
        client.name(),
        soroban_sdk::String::from_str(&env, "AfriPay Loyalty Points")
    );
}

#[test]
fn test_symbol() {
    let (env, client, _) = setup();
    assert_eq!(
        client.symbol(),
        soroban_sdk::String::from_str(&env, "ALP")
    );
}

#[test]
fn test_decimals_is_zero() {
    let (_, client, _) = setup();
    assert_eq!(client.decimals(), 0);
}

// ── mint ──────────────────────────────────────────────────────────────────────

#[test]
fn test_mint_increases_balance_and_supply() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &50);
    assert_eq!(client.balance(&user), 50);
    assert_eq!(client.total_supply(), 50);
}

#[test]
fn test_mint_accumulates_across_calls() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &30);
    client.mint(&admin, &user, &70);
    assert_eq!(client.balance(&user), 100);
    assert_eq!(client.total_supply(), 100);
}

#[test]
fn test_mint_multiple_users_independent_balances() {
    let (env, client, admin) = setup();
    let u1 = Address::generate(&env);
    let u2 = Address::generate(&env);
    client.mint(&admin, &u1, &40);
    client.mint(&admin, &u2, &60);
    assert_eq!(client.balance(&u1), 40);
    assert_eq!(client.balance(&u2), 60);
    assert_eq!(client.total_supply(), 100);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_mint_non_admin_panics() {
    let (env, client, _) = setup();
    let impostor = Address::generate(&env);
    let user = Address::generate(&env);
    client.mint(&impostor, &user, &10);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_mint_zero_amount_panics() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_mint_negative_amount_panics() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &-1);
}

#[test]
fn test_mint_exactly_to_cap_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &500);
    client.mint(&admin, &user, &500);
    assert_eq!(client.total_supply(), 500);
    assert_eq!(client.balance(&user), 500);
}

#[test]
#[should_panic(expected = "minting would exceed max supply")]
fn test_mint_exceeds_cap_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &100);
    client.mint(&admin, &user, &101);
}

#[test]
#[should_panic(expected = "minting would exceed max supply")]
fn test_mint_cumulative_exceeds_cap_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin, &100);
    client.mint(&admin, &user, &60);
    client.mint(&admin, &user, &41); // 60 + 41 = 101 > 100
}

// ── burn ──────────────────────────────────────────────────────────────────────

#[test]
fn test_burn_decreases_balance_and_supply() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &100);
    client.burn(&user, &40);
    assert_eq!(client.balance(&user), 60);
    assert_eq!(client.total_supply(), 60);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_burn_more_than_balance_panics() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &50);
    client.burn(&user, &51);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_burn_zero_panics() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &10);
    client.burn(&user, &0);
}

// ── transfer ──────────────────────────────────────────────────────────────────

#[test]
fn test_transfer_moves_points_between_accounts() {
    let (env, client, admin) = setup();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &sender, &100);
    client.transfer(&sender, &receiver, &30);
    assert_eq!(client.balance(&sender), 70);
    assert_eq!(client.balance(&receiver), 30);
    assert_eq!(client.total_supply(), 100); // supply unchanged
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_transfer_insufficient_balance_panics() {
    let (env, client, admin) = setup();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &sender, &10);
    client.transfer(&sender, &receiver, &11);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_transfer_zero_panics() {
    let (env, client, admin) = setup();
    let sender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &sender, &10);
    client.transfer(&sender, &receiver, &0);
}

// ── approve / allowance / transfer_from ──────────────────────────────────────

#[test]
fn test_approve_sets_allowance() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &50, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 50);
}

#[test]
fn test_transfer_from_uses_allowance() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &40, &FAR_FUTURE);
    client.transfer_from(&spender, &owner, &receiver, &40);
    assert_eq!(client.balance(&owner), 60);
    assert_eq!(client.balance(&receiver), 40);
    assert_eq!(client.allowance(&owner, &spender), 0);
}

#[test]
#[should_panic(expected = "insufficient allowance")]
fn test_transfer_from_exceeds_allowance_panics() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &10, &FAR_FUTURE);
    client.transfer_from(&spender, &owner, &receiver, &11);
}

#[test]
fn test_burn_from_uses_allowance() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &30, &FAR_FUTURE);
    client.burn_from(&spender, &owner, &30);
    assert_eq!(client.balance(&owner), 70);
    assert_eq!(client.total_supply(), 70);
    assert_eq!(client.allowance(&owner, &spender), 0);
}

#[test]
#[should_panic(expected = "insufficient allowance")]
fn test_burn_from_exceeds_allowance_panics() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &5, &FAR_FUTURE);
    client.burn_from(&spender, &owner, &6);
}

// ── allowance expiry ──────────────────────────────────────────────────────────

#[test]
fn test_allowance_returns_zero_after_expiry() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    // approve with expiry 1 second in the future from BASE_TIMESTAMP
    client.approve(&owner, &spender, &50, &(BASE_TIMESTAMP + 1));
    assert_eq!(client.allowance(&owner, &spender), 50);
    // advance past expiry
    env.ledger().with_mut(|l| l.timestamp = BASE_TIMESTAMP + 2);
    assert_eq!(client.allowance(&owner, &spender), 0);
}

#[test]
#[should_panic(expected = "allowance expired")]
fn test_transfer_from_expired_allowance_panics() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    let receiver = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &50, &(BASE_TIMESTAMP + 1));
    env.ledger().with_mut(|l| l.timestamp = BASE_TIMESTAMP + 2);
    client.transfer_from(&spender, &owner, &receiver, &10);
}

#[test]
#[should_panic(expected = "allowance expired")]
fn test_burn_from_expired_allowance_panics() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    client.approve(&owner, &spender, &50, &(BASE_TIMESTAMP + 1));
    env.ledger().with_mut(|l| l.timestamp = BASE_TIMESTAMP + 2);
    client.burn_from(&spender, &owner, &10);
}

#[test]
#[should_panic(expected = "expires_at must be in the future")]
fn test_approve_past_expiry_panics() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &100);
    // BASE_TIMESTAMP - 1 is in the past
    client.approve(&owner, &spender, &50, &(BASE_TIMESTAMP - 1));
}

// ── redeem ────────────────────────────────────────────────────────────────────

#[test]
fn test_redeem_burns_100_points_and_returns_true() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &150);
    let result = client.redeem(&user);
    assert!(result);
    assert_eq!(client.balance(&user), 50);
    assert_eq!(client.total_supply(), 50);
}

#[test]
fn test_redeem_exactly_100_points_leaves_zero() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &100);
    let result = client.redeem(&user);
    assert!(result);
    assert_eq!(client.balance(&user), 0);
    assert_eq!(client.total_supply(), 0);
}

#[test]
fn test_redeem_insufficient_points_returns_false() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &99);
    let result = client.redeem(&user);
    assert!(!result);
    // Balance unchanged
    assert_eq!(client.balance(&user), 99);
    assert_eq!(client.total_supply(), 99);
}

#[test]
fn test_redeem_zero_balance_returns_false() {
    let (env, client, _) = setup();
    let user = Address::generate(&env);
    let result = client.redeem(&user);
    assert!(!result);
}

#[test]
fn test_redeem_can_be_called_multiple_times() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    client.mint(&admin, &user, &300);
    assert!(client.redeem(&user)); // 300 → 200
    assert!(client.redeem(&user)); // 200 → 100
    assert!(client.redeem(&user)); // 100 → 0
    assert!(!client.redeem(&user)); // 0 → false
    assert_eq!(client.balance(&user), 0);
}

// ── balance of unknown account ────────────────────────────────────────────────

#[test]
fn test_balance_unknown_account_is_zero() {
    let (env, client, _) = setup();
    let unknown = Address::generate(&env);
    assert_eq!(client.balance(&unknown), 0);
}

// ── earn rate: 1 point per 1 XLM ─────────────────────────────────────────────

#[test]
fn test_mint_one_point_per_xlm_volume() {
    let (env, client, admin) = setup();
    let user = Address::generate(&env);
    // Simulate a 50 XLM transaction → mint 50 points
    let xlm_amount: i128 = 50;
    client.mint(&admin, &user, &xlm_amount);
    assert_eq!(client.balance(&user), 50);
}

// ── total_supply consistency ──────────────────────────────────────────────────

#[test]
fn test_total_supply_consistency_after_mint_transfer_burn_redeem() {
    let (env, client, admin) = setup();
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.mint(&admin, &user1, &200);
    assert_eq!(client.total_supply(), client.balance(&user1) + client.balance(&user2));

    client.mint(&admin, &user2, &100);
    assert_eq!(client.total_supply(), client.balance(&user1) + client.balance(&user2));

    client.transfer(&user1, &user2, &50);
    assert_eq!(client.total_supply(), client.balance(&user1) + client.balance(&user2));

    client.burn(&user1, &30);
    assert_eq!(client.total_supply(), client.balance(&user1) + client.balance(&user2));

    assert!(client.redeem(&user2)); // burns 100 from user2
    assert_eq!(client.total_supply(), client.balance(&user1) + client.balance(&user2));
}

// ── Helper for new tests (passes all 3 initialize args correctly) ─────────────

fn setup_v2() -> (Env, LoyaltyTokenContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = BASE_TIMESTAMP);
    let contract_id = env.register_contract(None, LoyaltyTokenContract);
    let client = LoyaltyTokenContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    // Pass transfer_fee_bps = 0 (no fees); initialize requires 3 args.
    client.initialize(&admin, &DEFAULT_MAX_SUPPLY, &0u32);
    (env, client, admin)
}

// ── increase_allowance ────────────────────────────────────────────────────────

/// Increase from zero: allowance starts at 0, delta bumps it to delta.
#[test]
fn test_increase_allowance_from_zero() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    assert_eq!(client.allowance(&owner, &spender), 0);
    client.increase_allowance(&owner, &spender, &50);
    assert_eq!(client.allowance(&owner, &spender), 50);
}

/// Increase from non-zero: existing allowance accumulates correctly.
#[test]
fn test_increase_allowance_from_nonzero() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    client.approve(&owner, &spender, &30, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 30);

    client.increase_allowance(&owner, &spender, &20);
    assert_eq!(client.allowance(&owner, &spender), 50);
}

/// increase_allowance with zero delta must panic.
#[test]
#[should_panic(expected = "delta must be positive")]
fn test_increase_allowance_zero_delta_panics() {
    let (env, client, _admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.increase_allowance(&owner, &spender, &0);
}

// ── decrease_allowance ────────────────────────────────────────────────────────

/// Decrease to zero: delta equals the current allowance.
#[test]
fn test_decrease_allowance_to_zero() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    client.approve(&owner, &spender, &50, &FAR_FUTURE);
    client.decrease_allowance(&owner, &spender, &50);
    assert_eq!(client.allowance(&owner, &spender), 0);
}

/// Decrease below zero: delta exceeds current allowance → must panic.
#[test]
#[should_panic(expected = "delta exceeds current allowance")]
fn test_decrease_allowance_below_zero_panics() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    client.approve(&owner, &spender, &40, &FAR_FUTURE);
    // delta (41) > allowance (40) → panic
    client.decrease_allowance(&owner, &spender, &41);
}

/// Decrease from non-zero to a smaller non-zero value.
#[test]
fn test_decrease_allowance_partial() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    client.approve(&owner, &spender, &100, &FAR_FUTURE);
    client.decrease_allowance(&owner, &spender, &60);
    assert_eq!(client.allowance(&owner, &spender), 40);
}

/// Decrease with zero delta must panic.
#[test]
#[should_panic(expected = "delta must be positive")]
fn test_decrease_allowance_zero_delta_panics() {
    let (env, client, _admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.decrease_allowance(&owner, &spender, &0);
}

// ── approve race-condition guard ──────────────────────────────────────────────

/// approve must panic when a non-zero allowance already exists and the new
/// amount is also non-zero.
#[test]
#[should_panic(expected = "Reset to zero before setting new allowance")]
fn test_approve_race_guard_panics_when_nonzero_allowance_exists() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    // Set an initial allowance of 50.
    client.approve(&owner, &spender, &50, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 50);

    // Attempting to replace it directly with a new non-zero value must panic.
    client.approve(&owner, &spender, &80, &FAR_FUTURE);
}

/// approve must succeed when resetting to zero first, then setting a new value.
#[test]
fn test_approve_reset_to_zero_then_set_new_value_succeeds() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    // Set initial allowance.
    client.approve(&owner, &spender, &50, &FAR_FUTURE);
    // Reset to zero.
    client.approve(&owner, &spender, &0, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 0);
    // Now set a new non-zero value — this should succeed.
    client.approve(&owner, &spender, &80, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 80);
}

/// approve with amount = 0 is always allowed, even if allowance is non-zero
/// (this is how you revoke/reset).
#[test]
fn test_approve_zero_amount_always_allowed() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    client.approve(&owner, &spender, &50, &FAR_FUTURE);
    // Revoking with amount = 0 must not trigger the race guard.
    client.approve(&owner, &spender, &0, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 0);
}

/// approve on a fresh allowance (zero) with a non-zero amount must succeed.
#[test]
fn test_approve_on_zero_allowance_succeeds() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &200);

    assert_eq!(client.allowance(&owner, &spender), 0);
    client.approve(&owner, &spender, &100, &FAR_FUTURE);
    assert_eq!(client.allowance(&owner, &spender), 100);
}

// ── AllowanceChanged event data integrity ─────────────────────────────────────

/// Verify that increase_allowance correctly reflects old and new values by
/// chaining multiple calls and checking the final state.
#[test]
fn test_increase_allowance_chained_updates() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &500);

    client.increase_allowance(&owner, &spender, &10);
    assert_eq!(client.allowance(&owner, &spender), 10);

    client.increase_allowance(&owner, &spender, &25);
    assert_eq!(client.allowance(&owner, &spender), 35);

    client.increase_allowance(&owner, &spender, &15);
    assert_eq!(client.allowance(&owner, &spender), 50);
}

/// Verify decrease_allowance followed by increase_allowance round-trips.
#[test]
fn test_decrease_then_increase_allowance_round_trip() {
    let (env, client, admin) = setup_v2();
    let owner = Address::generate(&env);
    let spender = Address::generate(&env);
    client.mint(&admin, &owner, &500);

    client.approve(&owner, &spender, &100, &FAR_FUTURE);
    client.decrease_allowance(&owner, &spender, &60);
    assert_eq!(client.allowance(&owner, &spender), 40);

    client.increase_allowance(&owner, &spender, &30);
    assert_eq!(client.allowance(&owner, &spender), 70);
}
