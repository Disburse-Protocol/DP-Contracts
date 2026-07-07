#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn contract_registers() {
    let env = Env::default();
    // Confirms the contract compiles and can be registered with the host.
    // Real coverage (schedules, batching, disbursement) is tracked as
    // Wave issues — see CONTRIBUTING.md.
    let org_registry = Address::generate(&env);
    let token = Address::generate(&env);
    env.register(PayrollContract, (org_registry, token));
}

#[test]
fn pay_frequency_equality() {
    assert_eq!(PayFrequency::Weekly, PayFrequency::Weekly);
    assert_ne!(PayFrequency::Weekly, PayFrequency::Monthly);
}
