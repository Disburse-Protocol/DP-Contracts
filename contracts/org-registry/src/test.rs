#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    vec, IntoVal, Val,
};

fn setup(env: &Env) -> (Address, OrgRegistryContractClient<'_>) {
    let super_admin = Address::generate(env);
    let contract_id = env.register(OrgRegistryContract, (super_admin,));
    (
        contract_id.clone(),
        OrgRegistryContractClient::new(env, &contract_id),
    )
}

/// `env.events().all()` only reflects the most recent top-level contract
/// invocation, so this must be called immediately after the call whose
/// event is under test — before any further client calls.
fn assert_last_event(env: &Env, expected: (Address, Vec<Val>, Val)) {
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    assert_eq!(vec![env, events.get(0).unwrap()], vec![env, expected]);
}

#[test]
fn contract_registers() {
    let env = Env::default();
    let super_admin = Address::generate(&env);
    env.register(OrgRegistryContract, (super_admin,));
}

#[test]
fn create_org_stores_org_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = setup(&env);

    let admin = Address::generate(&env);
    let name = Symbol::new(&env, "acme");
    let org_id = client.create_org(&name, &admin);
    assert_eq!(org_id, 1);

    let topics: Vec<Val> = vec![&env, Symbol::new(&env, "OrgCreated").into_val(&env)];
    let data: Val = (org_id, name.clone(), admin.clone()).into_val(&env);
    assert_last_event(&env, (contract_id, topics, data));

    let org = client.get_org(&org_id);
    assert_eq!(org.org_id, 1);
    assert_eq!(org.name, name);
    assert_eq!(org.admin, admin);
    assert_eq!(org.signers, Vec::from_array(&env, [admin]));
    assert_eq!(org.employee_count, 0);
}

#[test]
fn add_employee_increments_count_once_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = setup(&env);

    let admin = Address::generate(&env);
    let org_id = client.create_org(&Symbol::new(&env, "acme"), &admin);

    let employee = Address::generate(&env);
    let display_name = Symbol::new(&env, "alice");
    let role = Symbol::new(&env, "engineer");
    client.add_employee(&org_id, &employee, &display_name, &role);

    let topics: Vec<Val> = vec![&env, Symbol::new(&env, "EmployeeAdded").into_val(&env)];
    let data: Val = (org_id, employee.clone(), display_name.clone(), role.clone()).into_val(&env);
    assert_last_event(&env, (contract_id, topics, data));

    assert_eq!(client.get_org(&org_id).employee_count, 1);

    // Re-adding the same employee updates their record but does not
    // double-count or re-emit EmployeeAdded.
    client.add_employee(&org_id, &employee, &display_name, &role);
    assert_eq!(env.events().all().len(), 0);
    assert_eq!(client.get_org(&org_id).employee_count, 1);
}

#[test]
fn remove_employee_deactivates_and_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = setup(&env);

    let admin = Address::generate(&env);
    let org_id = client.create_org(&Symbol::new(&env, "acme"), &admin);
    let employee = Address::generate(&env);
    client.add_employee(
        &org_id,
        &employee,
        &Symbol::new(&env, "alice"),
        &Symbol::new(&env, "engineer"),
    );

    client.remove_employee(&org_id, &employee);

    let topics: Vec<Val> = vec![&env, Symbol::new(&env, "EmployeeRemoved").into_val(&env)];
    let data: Val = (org_id, employee.clone()).into_val(&env);
    assert_last_event(&env, (contract_id, topics, data));

    assert_eq!(client.get_org(&org_id).employee_count, 0);
    assert!(!client.get_employee(&org_id, &employee).active);
}

#[test]
fn add_employee_rejects_non_admin_caller() {
    let env = Env::default();
    let (_, client) = setup(&env);

    let admin = Address::generate(&env);
    env.mock_all_auths();
    let org_id = client.create_org(&Symbol::new(&env, "acme"), &admin);

    // Without mock_all_auths, the admin's require_auth() has nothing to
    // authorize against and the call must fail.
    env.set_auths(&[]);
    let employee = Address::generate(&env);
    let result = client.try_add_employee(
        &org_id,
        &employee,
        &Symbol::new(&env, "alice"),
        &Symbol::new(&env, "engineer"),
    );
    assert!(result.is_err());
}

#[test]
fn get_org_not_found_errors() {
    let env = Env::default();
    let (_, client) = setup(&env);
    let result = client.try_get_org(&999);
    assert_eq!(result, Err(Ok(OrgRegistryError::OrgNotFound)));
}
