#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn contract_registers() {
    let env = Env::default();
    // Confirms the contract compiles and can be registered with the host.
    // Real coverage (schedules, batching, disbursement) is tracked as
    // Wave issues — see CONTRIBUTING.md.
    env.register(PayrollContract, ());
}

#[test]
fn pay_frequency_equality() {
    assert_eq!(PayFrequency::Weekly, PayFrequency::Weekly);
    assert_ne!(PayFrequency::Weekly, PayFrequency::Monthly);
}
