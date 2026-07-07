§#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Organization {
    pub org_id: u64,
    pub name: Symbol,
    pub admin: Address,
    pub signers: Vec<Address>,
    pub employee_count: u32,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Employee {
    pub address: Address,
    pub display_name: Symbol,
    pub role: Symbol,
    pub added_at: u64,
    pub active: bool,
}

#[contracttype]
pub enum DataKey {
    NextOrgId,
    Org(u64),               // org_id -> Organization
    Employee(u64, Address), // (org_id, employee) -> Employee
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OrgRegistryError {
    OrgNotFound = 1,
    EmployeeNotFound = 2,
    NotAuthorized = 3,
    CannotRemoveLastSigner = 4,
}

#[contract]
pub struct OrgRegistryContract;

#[contractimpl]
impl OrgRegistryContract {
    /// Creates an organization. Caller becomes admin. Returns org_id.
    pub fn create_org(_env: Env, _name: Symbol, _admin: Address) -> Result<u64, OrgRegistryError> {
        unimplemented!("Wave task: create_org — see contracts repo issue tracker")
    }

    /// Adds an address as a payroll approver.
    pub fn add_signer(_env: Env, _org_id: u64, _signer: Address) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: add_signer — see contracts repo issue tracker")
    }

    /// Removes a signer. Cannot remove last signer.
    pub fn remove_signer(
        _env: Env,
        _org_id: u64,
        _signer: Address,
    ) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: remove_signer — see contracts repo issue tracker")
    }

    /// Registers an employee in the org.
    pub fn add_employee(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _display_name: Symbol,
        _role: Symbol,
    ) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: add_employee — see contracts repo issue tracker")
    }

    /// Deactivates employee. Triggers schedule removal in payroll contract.
    pub fn remove_employee(
        _env: Env,
        _org_id: u64,
        _employee: Address,
    ) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: remove_employee — see contracts repo issue tracker")
    }

    /// Updates employee metadata.
    pub fn update_employee(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _display_name: Symbol,
        _role: Symbol,
    ) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: update_employee — see contracts repo issue tracker")
    }

    /// Transfers admin role.
    pub fn transfer_admin(
        _env: Env,
        _org_id: u64,
        _new_admin: Address,
    ) -> Result<(), OrgRegistryError> {
        unimplemented!("Wave task: transfer_admin — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_org(_env: Env, _org_id: u64) -> Result<Organization, OrgRegistryError> {
        unimplemented!("Wave task: get_org — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_employee(
        _env: Env,
        _org_id: u64,
        _employee: Address,
    ) -> Result<Employee, OrgRegistryError> {
        unimplemented!("Wave task: get_employee — see contracts repo issue tracker")
    }

    /// Read-only. All employees in org.
    pub fn get_employees(_env: Env, _org_id: u64) -> Result<Vec<Employee>, OrgRegistryError> {
        unimplemented!("Wave task: get_employees — see contracts repo issue tracker")
    }

    /// Used by payroll contract to validate batch approvals.
    pub fn is_signer(_env: Env, _org_id: u64, _address: Address) -> bool {
        unimplemented!("Wave task: is_signer — see contracts repo issue tracker")
    }

    /// Used by payroll contract to validate admin actions.
    pub fn is_admin(_env: Env, _org_id: u64, _address: Address) -> bool {
        unimplemented!("Wave task: is_admin — see contracts repo issue tracker")
    }
}

mod test;
