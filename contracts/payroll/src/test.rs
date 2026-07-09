#![cfg(test)]

use super::*;
use org_registry::OrgRegistryContract;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token, vec, IntoVal, Val,
};

struct Fixture<'a> {
    contract_id: Address,
    client: PayrollContractClient<'a>,
    token_client: token::Client<'a>,
    token_admin: token::StellarAssetClient<'a>,
    org_id: u64,
    org_admin: Address,
}

fn setup(env: &Env) -> Fixture<'_> {
    env.mock_all_auths();

    let super_admin = Address::generate(env);
    let org_registry_id = env.register(OrgRegistryContract, (super_admin,));
    let org_registry_client = org_registry::OrgRegistryContractClient::new(env, &org_registry_id);

    let org_admin = Address::generate(env);
    let org_id = org_registry_client.create_org(&Symbol::new(env, "acme"), &org_admin);

    let token_admin_addr = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(token_admin_addr.clone());
    let token_id = sac.address();

    let contract_id = env.register(PayrollContract, (org_registry_id, token_id.clone()));
    let client = PayrollContractClient::new(env, &contract_id);

    Fixture {
        contract_id,
        client,
        token_client: token::Client::new(env, &token_id),
        token_admin: token::StellarAssetClient::new(env, &token_id),
        org_id,
        org_admin,
    }
}

/// `env.events().all()` only reflects the most recent top-level contract
/// invocation, so this must be called immediately after the call whose
/// event is under test — before any further client calls (including reads).
/// The token contract also emits its own events for transfers, so this
/// checks the *last* published event rather than assuming there's only one.
fn assert_last_event(env: &Env, expected: (Address, Vec<Val>, Val)) {
    let events = env.events().all();
    assert!(!events.is_empty());
    let actual = events.get(events.len() - 1).unwrap();
    assert_eq!(vec![env, actual], vec![env, expected]);
}

#[test]
fn contract_registers() {
    let env = Env::default();
    let org_registry = Address::generate(&env);
    let token = Address::generate(&env);
    env.register(PayrollContract, (org_registry, token));
}

#[test]
fn pay_frequency_equality() {
    assert_eq!(PayFrequency::Weekly, PayFrequency::Weekly);
    assert_ne!(PayFrequency::Weekly, PayFrequency::Monthly);
}

#[test]
fn fund_payroll_increases_balance_and_emits_event() {
    let env = Env::default();
    let f = setup(&env);

    let funder = Address::generate(&env);
    f.token_admin.mint(&funder, &1_000_000);

    f.client.fund_payroll(&f.org_id, &funder, &500_000);

    let topics: Vec<Val> = vec![&env, Symbol::new(&env, "PayrollFunded").into_val(&env)];
    let data: Val = (f.org_id, funder, 500_000i128, 500_000i128).into_val(&env);
    assert_last_event(&env, (f.contract_id.clone(), topics, data));

    assert_eq!(f.client.get_org_balance(&f.org_id), 500_000);
}

#[test]
fn add_schedule_rejects_bad_split_sum() {
    let env = Env::default();
    let f = setup(&env);

    let employee = Address::generate(&env);
    let bad_splits = vec![
        &env,
        PaySplit {
            destination: employee.clone(),
            percentage: 5_000,
        },
    ];
    let result = f.client.try_add_schedule(
        &f.org_id,
        &employee,
        &2_000_i128,
        &PayFrequency::Monthly,
        &bad_splits,
        &f.org_admin,
    );
    assert_eq!(result, Err(Ok(PayrollError::InvalidSplits)));
}

#[test]
fn add_schedule_stores_schedule_and_emits_event() {
    let env = Env::default();
    let f = setup(&env);

    let employee = Address::generate(&env);
    let splits = vec![
        &env,
        PaySplit {
            destination: employee.clone(),
            percentage: 10_000,
        },
    ];
    f.client.add_schedule(
        &f.org_id,
        &employee,
        &2_000_i128,
        &PayFrequency::Monthly,
        &splits,
        &f.org_admin,
    );

    let topics: Vec<Val> = vec![&env, Symbol::new(&env, "ScheduleAdded").into_val(&env)];
    let data: Val = (
        f.org_id,
        employee.clone(),
        2_000_i128,
        PayFrequency::Monthly,
    )
        .into_val(&env);
    assert_last_event(&env, (f.contract_id.clone(), topics, data));

    let schedule = f.client.get_schedule(&f.org_id, &employee);
    assert_eq!(schedule.amount, 2_000);
    assert!(schedule.active);
    assert_eq!(schedule.total_paid, 0);
}

#[test]
fn execute_payroll_disburses_splits_and_emits_events() {
    let env = Env::default();
    let f = setup(&env);

    let funder = Address::generate(&env);
    f.token_admin.mint(&funder, &1_000_000);
    f.client.fund_payroll(&f.org_id, &funder, &10_000);

    let employee = Address::generate(&env);
    let main_wallet = Address::generate(&env);
    let savings_wallet = Address::generate(&env);
    let splits = vec![
        &env,
        PaySplit {
            destination: main_wallet.clone(),
            percentage: 7_000,
        },
        PaySplit {
            destination: savings_wallet.clone(),
            percentage: 3_000,
        },
    ];
    f.client.add_schedule(
        &f.org_id,
        &employee,
        &2_000_i128,
        &PayFrequency::Monthly,
        &splits,
        &f.org_admin,
    );

    let batch_id = f.client.prepare_batch(&f.org_id);
    f.client.execute_payroll(&f.org_id, &batch_id);

    // Capture events immediately — before any further client calls, which
    // would each start a new top-level invocation and clear this buffer.
    // execute_payroll's invocation contains, in order: two token transfer
    // events (one per split), one EmployeePaid, then one PayrollExecuted.
    let events = env.events().all();
    assert_eq!(events.len(), 4);
    let paid_topics: Vec<Val> = vec![&env, Symbol::new(&env, "EmployeePaid").into_val(&env)];
    let paid_data: Val = (f.org_id, employee.clone(), 2_000_i128).into_val(&env);
    assert_eq!(
        vec![&env, events.get(2).unwrap()],
        vec![&env, (f.contract_id.clone(), paid_topics, paid_data)]
    );
    let executed_topics: Vec<Val> = vec![&env, Symbol::new(&env, "PayrollExecuted").into_val(&env)];
    let executed_data: Val = (f.org_id, batch_id, 2_000_i128, 1u32).into_val(&env);
    assert_eq!(
        vec![&env, events.get(3).unwrap()],
        vec![&env, (f.contract_id, executed_topics, executed_data)]
    );

    // Split math: 2,000 @ 70/30.
    assert_eq!(f.token_client.balance(&main_wallet), 1_400);
    assert_eq!(f.token_client.balance(&savings_wallet), 600);

    let schedule = f.client.get_schedule(&f.org_id, &employee);
    assert_eq!(schedule.total_paid, 2_000);
    assert!(schedule.next_payment_at > 0);

    assert_eq!(f.client.get_org_balance(&f.org_id), 8_000);
}

#[test]
fn execute_payroll_fails_on_insufficient_balance() {
    let env = Env::default();
    let f = setup(&env);

    let employee = Address::generate(&env);
    let splits = vec![
        &env,
        PaySplit {
            destination: employee.clone(),
            percentage: 10_000,
        },
    ];
    f.client.add_schedule(
        &f.org_id,
        &employee,
        &2_000_i128,
        &PayFrequency::Monthly,
        &splits,
        &f.org_admin,
    );

    // No fund_payroll call — balance stays at 0, below the 2,000 owed.
    let batch_id = f.client.prepare_batch(&f.org_id);
    let result = f.client.try_execute_payroll(&f.org_id, &batch_id);
    assert_eq!(result, Err(Ok(PayrollError::InsufficientBalance)));
}
