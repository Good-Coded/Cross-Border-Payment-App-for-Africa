#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal, Symbol, Val,
};

use crate::{EvtFeeDeposited, EvtFeesWithdrawn, FeeDistributorContract, FeeDistributorContractClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, FeeDistributorContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, FeeDistributorContract);
    let client = FeeDistributorContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    let usdc_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
    client.initialize(&admin, &usdc_id);
    (env, client, admin, usdc_id)
}

fn mint(env: &Env, usdc_id: &Address, to: &Address, amount: i128) {
    StellarAssetClient::new(env, usdc_id).mint(to, &amount);
}

// ── initialize ────────────────────────────────────────────────────────────────

#[test]
fn test_initial_fees_are_zero() {
    let (_, client, _, _) = setup();
    assert_eq!(client.get_accumulated_fees(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_, client, admin, usdc_id) = setup();
    client.initialize(&admin, &usdc_id);
}

// ── deposit_fee ───────────────────────────────────────────────────────────────

#[test]
fn test_deposit_fee_increments_total() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &500_0000000, &None);
    assert_eq!(client.get_accumulated_fees(), 500_0000000);
}

#[test]
fn test_deposit_fee_transfers_usdc_to_contract() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &1_000_0000000, &None);
    assert_eq!(TokenClient::new(&env, &usdc_id).balance(&depositor), 0);
}

#[test]
fn test_multiple_deposits_accumulate() {
    let (env, client, _, usdc_id) = setup();
    let d1 = Address::generate(&env);
    let d2 = Address::generate(&env);
    mint(&env, &usdc_id, &d1, 300_0000000);
    mint(&env, &usdc_id, &d2, 200_0000000);
    client.deposit_fee(&d1, &300_0000000, &None);
    client.deposit_fee(&d2, &200_0000000, &None);
    assert_eq!(client.get_accumulated_fees(), 500_0000000);
}

#[test]
fn test_deposit_fee_minimum_amount() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1);
    client.deposit_fee(&depositor, &1, &None);
    assert_eq!(client.get_accumulated_fees(), 1);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_deposit_fee_zero_panics() {
    let (env, client, _, _) = setup();
    let depositor = Address::generate(&env);
    client.deposit_fee(&depositor, &0, &None);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_deposit_fee_negative_panics() {
    let (env, client, _, _) = setup();
    let depositor = Address::generate(&env);
    client.deposit_fee(&depositor, &-1, &None);
}

// ── #557: deposit source tracking ─────────────────────────────────────────────

#[test]
fn test_deposit_fee_with_source_emits_event() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    let source = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    client.deposit_fee(&depositor, &500_0000000, &Some(source.clone()));

    let event_name: Val = Symbol::new(&env, "FeeDeposited").into_val(&env);
    let events = env.events().all();
    let deposit_event = events.iter().find(|(_, topics, _)| {
        topics.iter().any(|t| t == &event_name)
    });
    assert!(deposit_event.is_some(), "FeeDeposited event not emitted");

    let (_, _, data) = deposit_event.unwrap();
    let payload: EvtFeeDeposited = soroban_sdk::from_val(&env, data);
    assert_eq!(payload.depositor, depositor);
    assert_eq!(payload.amount, 500_0000000);
    assert_eq!(payload.source, Some(source));
}

#[test]
fn test_deposit_fee_without_source_has_none() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 100_0000000);
    client.deposit_fee(&depositor, &100_0000000, &None);

    let event_name: Val = Symbol::new(&env, "FeeDeposited").into_val(&env);
    let events = env.events().all();
    let deposit_event = events.iter().find(|(_, topics, _)| {
        topics.iter().any(|t| t == &event_name)
    });
    assert!(deposit_event.is_some());

    let (_, _, data) = deposit_event.unwrap();
    let payload: EvtFeeDeposited = soroban_sdk::from_val(&env, data);
    assert_eq!(payload.source, None);
}

// ── withdraw_fees ─────────────────────────────────────────────────────────────

#[test]
fn test_withdraw_fees_transfers_to_admin() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &1_000_0000000, &None);

    client.withdraw_fees(&admin, &1_000_0000000);

    assert_eq!(client.get_accumulated_fees(), 0);
    assert_eq!(TokenClient::new(&env, &usdc_id).balance(&admin), 1_000_0000000);
}

#[test]
fn test_withdraw_fees_partial() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &1_000_0000000, &None);

    client.withdraw_fees(&admin, &400_0000000);

    assert_eq!(client.get_accumulated_fees(), 600_0000000);
    assert_eq!(TokenClient::new(&env, &usdc_id).balance(&admin), 400_0000000);
}

#[test]
fn test_withdraw_fees_multiple_times() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &1_000_0000000, &None);

    client.withdraw_fees(&admin, &300_0000000);
    client.withdraw_fees(&admin, &300_0000000);

    assert_eq!(client.get_accumulated_fees(), 400_0000000);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_withdraw_fees_non_admin_panics() {
    let (env, client, _, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    client.deposit_fee(&depositor, &500_0000000, &None);
    let impostor = Address::generate(&env);
    client.withdraw_fees(&impostor, &100_0000000);
}

#[test]
#[should_panic(expected = "insufficient accumulated fees")]
fn test_withdraw_fees_exceeds_balance_panics() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 100_0000000);
    client.deposit_fee(&depositor, &100_0000000, &None);
    client.withdraw_fees(&admin, &100_0000001);
}

#[test]
#[should_panic(expected = "insufficient accumulated fees")]
fn test_withdraw_fees_when_empty_panics() {
    let (_, client, admin, _) = setup();
    client.withdraw_fees(&admin, &1);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_withdraw_fees_zero_panics() {
    let (_, client, admin, _) = setup();
    client.withdraw_fees(&admin, &0);
}

// ── #556: withdrawal history event includes timestamp ─────────────────────────

#[test]
fn test_withdraw_fees_event_includes_admin_amount_and_timestamp() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);
    client.deposit_fee(&depositor, &1_000_0000000, &None);

    let withdraw_at: u64 = 99_999;
    env.ledger().with_mut(|li| li.timestamp = withdraw_at);
    client.withdraw_fees(&admin, &1_000_0000000);

    let event_name: Val = Symbol::new(&env, "FeesWithdrawn").into_val(&env);
    let events = env.events().all();
    let withdrawal_event = events.iter().find(|(_, topics, _)| {
        topics.iter().any(|t| t == &event_name)
    });
    assert!(withdrawal_event.is_some(), "FeesWithdrawn event not emitted");

    let (_, _, data) = withdrawal_event.unwrap();
    let payload: EvtFeesWithdrawn = soroban_sdk::from_val(&env, data);
    assert_eq!(payload.admin, admin);
    assert_eq!(payload.amount, 1_000_0000000);
    assert_eq!(payload.remaining, 0);
    assert_eq!(payload.timestamp, withdraw_at);
}

// ── get_accumulated_fees ──────────────────────────────────────────────────────

#[test]
fn test_get_accumulated_fees_reflects_deposits_and_withdrawals() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 1_000_0000000);

    client.deposit_fee(&depositor, &600_0000000, &None);
    assert_eq!(client.get_accumulated_fees(), 600_0000000);

    client.withdraw_fees(&admin, &200_0000000);
    assert_eq!(client.get_accumulated_fees(), 400_0000000);

    client.deposit_fee(&depositor, &400_0000000, &None);
    assert_eq!(client.get_accumulated_fees(), 800_0000000);
}

// ── circuit-breaker pause ─────────────────────────────────────────────────────

#[test]
fn test_is_paused_false_by_default() {
    let (_, client, _, _) = setup();
    assert!(!client.is_paused());
}

#[test]
fn test_pause_sets_paused_flag() {
    let (_, client, admin, _) = setup();
    client.pause(&admin);
    assert!(client.is_paused());
}

#[test]
fn test_unpause_clears_paused_flag() {
    let (_, client, admin, _) = setup();
    client.pause(&admin);
    client.unpause(&admin);
    assert!(!client.is_paused());
}

#[test]
fn test_pause_unpause_cycle() {
    let (_, client, admin, _) = setup();
    // starts unpaused
    assert!(!client.is_paused());
    // pause
    client.pause(&admin);
    assert!(client.is_paused());
    // unpause
    client.unpause(&admin);
    assert!(!client.is_paused());
    // can pause again
    client.pause(&admin);
    assert!(client.is_paused());
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_deposit_fee_panics_when_paused() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    client.pause(&admin);
    client.deposit_fee(&depositor, &500_0000000, &None);
}

#[test]
#[should_panic(expected = "Contract is paused")]
fn test_withdraw_fees_panics_when_paused() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    // deposit while unpaused so there are funds to attempt withdrawal
    client.deposit_fee(&depositor, &500_0000000, &None);
    client.pause(&admin);
    client.withdraw_fees(&admin, &500_0000000);
}

#[test]
fn test_get_accumulated_fees_readable_when_paused() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 300_0000000);
    client.deposit_fee(&depositor, &300_0000000, &None);
    client.pause(&admin);
    // read-only operation must remain accessible
    assert_eq!(client.get_accumulated_fees(), 300_0000000);
}

#[test]
fn test_is_paused_readable_when_paused() {
    let (_, client, admin, _) = setup();
    client.pause(&admin);
    // is_paused itself must be callable when paused
    assert!(client.is_paused());
}

#[test]
fn test_deposit_succeeds_after_unpause() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    client.pause(&admin);
    client.unpause(&admin);
    // should not panic now
    client.deposit_fee(&depositor, &500_0000000, &None);
    assert_eq!(client.get_accumulated_fees(), 500_0000000);
}

#[test]
fn test_withdraw_succeeds_after_unpause() {
    let (env, client, admin, usdc_id) = setup();
    let depositor = Address::generate(&env);
    mint(&env, &usdc_id, &depositor, 500_0000000);
    client.deposit_fee(&depositor, &500_0000000, &None);
    client.pause(&admin);
    client.unpause(&admin);
    // should not panic now
    client.withdraw_fees(&admin, &500_0000000);
    assert_eq!(client.get_accumulated_fees(), 0);
}

#[test]
fn test_pause_emits_contract_paused_event() {
    let (env, client, admin, _) = setup();
    let pause_at: u64 = 12_345;
    env.ledger().with_mut(|li| li.timestamp = pause_at);
    client.pause(&admin);

    let event_name: Val = Symbol::new(&env, "ContractPaused").into_val(&env);
    let events = env.events().all();
    let pause_event = events.iter().find(|(_, topics, _)| {
        topics.iter().any(|t| t == &event_name)
    });
    assert!(pause_event.is_some(), "ContractPaused event not emitted");

    let (_, _, data) = pause_event.unwrap();
    let payload: crate::EvtContractPaused = soroban_sdk::from_val(&env, data);
    assert_eq!(payload.admin, admin);
    assert_eq!(payload.paused_at, pause_at);
}

#[test]
fn test_unpause_emits_contract_unpaused_event() {
    let (env, client, admin, _) = setup();
    client.pause(&admin);

    let unpause_at: u64 = 99_000;
    env.ledger().with_mut(|li| li.timestamp = unpause_at);
    client.unpause(&admin);

    let event_name: Val = Symbol::new(&env, "ContractUnpaused").into_val(&env);
    let events = env.events().all();
    let unpause_event = events.iter().find(|(_, topics, _)| {
        topics.iter().any(|t| t == &event_name)
    });
    assert!(unpause_event.is_some(), "ContractUnpaused event not emitted");

    let (_, _, data) = unpause_event.unwrap();
    let payload: crate::EvtContractUnpaused = soroban_sdk::from_val(&env, data);
    assert_eq!(payload.admin, admin);
    assert_eq!(payload.unpaused_at, unpause_at);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_pause_non_admin_panics() {
    let (env, client, _, _) = setup();
    let impostor = Address::generate(&env);
    client.pause(&impostor);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_unpause_non_admin_panics() {
    let (env, client, admin, _) = setup();
    client.pause(&admin);
    let impostor = Address::generate(&env);
    client.unpause(&impostor);
}
