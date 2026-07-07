#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn contract_registers() {
    let env = Env::default();
    let org_registry = Address::generate(&env);
    env.register(VestingContract, (org_registry,));
}
