#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Env, Symbol,
    Vec,
};

/// Ledgers to extend persistent entries by on every write (~30 days at 5s/ledger).
const TTL_EXTEND_TO: u32 = 518_400;
/// Extend once the entry's remaining TTL drops below this threshold.
const TTL_THRESHOLD: u32 = 100_000;

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
    SuperAdmin,
    PayrollContract,
    NextOrgId,
    Org(u64),
    Employee(u64, Address),
    EmployeeList(u64), // org_id -> Vec<Address>, all employees ever added (active or not)
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OrgRegistryError {
    OrgNotFound = 1,
    EmployeeNotFound = 2,
    NotAuthorized = 3,
    CannotRemoveLastSigner = 4,
    AlreadySet = 5,
}

/// Minimal view of the Payroll contract used for the employee-removal cascade.
/// Cross-contract client generated from this trait — see the `payroll` crate
/// for the authoritative implementation.
#[contractclient(name = "PayrollClient")]
pub trait PayrollInterface {
    fn remove_schedule(env: Env, org_id: u64, employee: Address, caller: Address);
}

#[contract]
pub struct OrgRegistryContract;

#[contractimpl]
impl OrgRegistryContract {
    /// `super_admin` has no power over individual orgs or their funds — it is
    /// only used to authorize wiring the Payroll contract address once, after
    /// deployment, via [`Self::set_payroll_contract`].
    pub fn __constructor(env: Env, super_admin: Address) {
        env.storage().instance().set(&DataKey::SuperAdmin, &super_admin);
    }

    /// One-time wiring of the deployed Payroll contract's address, used to
    /// cascade employee removal into `payroll.remove_schedule`.
    pub fn set_payroll_contract(
        env: Env,
        payroll_contract: Address,
    ) -> Result<(), OrgRegistryError> {
        let super_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::SuperAdmin)
            .ok_or(OrgRegistryError::NotAuthorized)?;
        super_admin.require_auth();

        if env.storage().instance().has(&DataKey::PayrollContract) {
            return Err(OrgRegistryError::AlreadySet);
        }
        env.storage()
            .instance()
            .set(&DataKey::PayrollContract, &payroll_contract);
        Ok(())
    }

    /// Creates an organization. Caller becomes admin. Returns org_id.
    pub fn create_org(env: Env, name: Symbol, admin: Address) -> Result<u64, OrgRegistryError> {
        admin.require_auth();

        let next_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextOrgId)
            .unwrap_or(0)
            + 1;

        let org = Organization {
            org_id: next_id,
            name,
            admin: admin.clone(),
            signers: Vec::from_array(&env, [admin]),
            employee_count: 0,
            created_at: env.ledger().timestamp(),
        };

        env.storage().instance().set(&DataKey::NextOrgId, &next_id);
        Self::write_org(&env, &org);
        env.storage()
            .persistent()
            .set(&DataKey::EmployeeList(next_id), &Vec::<Address>::new(&env));
        env.storage().persistent().extend_ttl(
            &DataKey::EmployeeList(next_id),
            TTL_THRESHOLD,
            TTL_EXTEND_TO,
        );

        Ok(next_id)
    }

    /// Adds an address as a payroll approver.
    pub fn add_signer(env: Env, org_id: u64, signer: Address) -> Result<(), OrgRegistryError> {
        let mut org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        if !org.signers.contains(&signer) {
            org.signers.push_back(signer);
        }
        Self::write_org(&env, &org);
        Ok(())
    }

    /// Removes a signer. Cannot remove last signer.
    pub fn remove_signer(env: Env, org_id: u64, signer: Address) -> Result<(), OrgRegistryError> {
        let mut org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        if org.signers.len() <= 1 {
            return Err(OrgRegistryError::CannotRemoveLastSigner);
        }
        if let Some(index) = org.signers.iter().position(|s| s == signer) {
            org.signers.remove(index as u32);
        }
        Self::write_org(&env, &org);
        Ok(())
    }

    /// Registers an employee in the org.
    pub fn add_employee(
        env: Env,
        org_id: u64,
        employee: Address,
        display_name: Symbol,
        role: Symbol,
    ) -> Result<(), OrgRegistryError> {
        let mut org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        let key = DataKey::Employee(org_id, employee.clone());
        let is_new = !env.storage().persistent().has(&key);

        let record = Employee {
            address: employee.clone(),
            display_name,
            role,
            added_at: env.ledger().timestamp(),
            active: true,
        };
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);

        if is_new {
            let list_key = DataKey::EmployeeList(org_id);
            let mut list: Vec<Address> = env
                .storage()
                .persistent()
                .get(&list_key)
                .unwrap_or_else(|| Vec::new(&env));
            list.push_back(employee);
            env.storage().persistent().set(&list_key, &list);
            env.storage()
                .persistent()
                .extend_ttl(&list_key, TTL_THRESHOLD, TTL_EXTEND_TO);

            org.employee_count += 1;
            Self::write_org(&env, &org);
        }

        Ok(())
    }

    /// Deactivates employee. Triggers schedule removal in payroll contract.
    pub fn remove_employee(
        env: Env,
        org_id: u64,
        employee: Address,
    ) -> Result<(), OrgRegistryError> {
        let mut org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        let key = DataKey::Employee(org_id, employee.clone());
        let mut record = env
            .storage()
            .persistent()
            .get::<_, Employee>(&key)
            .ok_or(OrgRegistryError::EmployeeNotFound)?;

        if record.active {
            record.active = false;
            env.storage().persistent().set(&key, &record);

            org.employee_count = org.employee_count.saturating_sub(1);
            Self::write_org(&env, &org);
        }

        if let Some(payroll_contract) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::PayrollContract)
        {
            let client = PayrollClient::new(&env, &payroll_contract);
            let _ = client.try_remove_schedule(
                &org_id,
                &employee,
                &env.current_contract_address(),
            );
        }

        Ok(())
    }

    /// Updates employee metadata.
    pub fn update_employee(
        env: Env,
        org_id: u64,
        employee: Address,
        display_name: Symbol,
        role: Symbol,
    ) -> Result<(), OrgRegistryError> {
        let org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        let key = DataKey::Employee(org_id, employee);
        let mut record = env
            .storage()
            .persistent()
            .get::<_, Employee>(&key)
            .ok_or(OrgRegistryError::EmployeeNotFound)?;
        record.display_name = display_name;
        record.role = role;
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Transfers admin role.
    pub fn transfer_admin(
        env: Env,
        org_id: u64,
        new_admin: Address,
    ) -> Result<(), OrgRegistryError> {
        let mut org = Self::read_org(&env, org_id)?;
        org.admin.require_auth();

        org.admin = new_admin.clone();
        if !org.signers.contains(&new_admin) {
            org.signers.push_back(new_admin);
        }
        Self::write_org(&env, &org);
        Ok(())
    }

    /// Read-only.
    pub fn get_org(env: Env, org_id: u64) -> Result<Organization, OrgRegistryError> {
        Self::read_org(&env, org_id)
    }

    /// Read-only.
    pub fn get_employee(
        env: Env,
        org_id: u64,
        employee: Address,
    ) -> Result<Employee, OrgRegistryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Employee(org_id, employee))
            .ok_or(OrgRegistryError::EmployeeNotFound)
    }

    /// Read-only. All employees ever added to the org (active or not).
    pub fn get_employees(env: Env, org_id: u64) -> Result<Vec<Employee>, OrgRegistryError> {
        let addresses: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::EmployeeList(org_id))
            .unwrap_or_else(|| Vec::new(&env));

        let mut employees = Vec::new(&env);
        for address in addresses.iter() {
            if let Some(employee) = env
                .storage()
                .persistent()
                .get::<_, Employee>(&DataKey::Employee(org_id, address))
            {
                employees.push_back(employee);
            }
        }
        Ok(employees)
    }

    /// Used by payroll contract to validate batch approvals.
    pub fn is_signer(env: Env, org_id: u64, address: Address) -> bool {
        match Self::read_org(&env, org_id) {
            Ok(org) => org.signers.contains(&address),
            Err(_) => false,
        }
    }

    /// Used by payroll contract to validate admin actions.
    pub fn is_admin(env: Env, org_id: u64, address: Address) -> bool {
        match Self::read_org(&env, org_id) {
            Ok(org) => org.admin == address,
            Err(_) => false,
        }
    }

    fn read_org(env: &Env, org_id: u64) -> Result<Organization, OrgRegistryError> {
        env.storage()
            .persistent()
            .get(&DataKey::Org(org_id))
            .ok_or(OrgRegistryError::OrgNotFound)
    }

    fn write_org(env: &Env, org: &Organization) {
        let key = DataKey::Org(org.org_id);
        env.storage().persistent().set(&key, org);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

mod test;
