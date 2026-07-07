#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

#[test]
fn contract_registers() {
    let env = Env::default();
    let super_admin = Address::generate(&env);
    env.register(OrgRegistryContract, (super_admin,));
}
