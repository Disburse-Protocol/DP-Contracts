#![no_std]
// create_grant's 7 params (env, org_id, employee, token, amount, cliff_seconds,
// vesting_seconds, caller) mirror the on-chain VestingGrant fields one-to-one;
// splitting them into a struct would just move the field list, not shrink it.
// The lint also fires on the `#[contractimpl]`/`#[contractclient]`-generated
// client code for this function, which an attribute on the impl block alone
// doesn't reach, hence the crate-level allow.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, token, Address, Env, Vec,
};

/// Ledgers to extend persistent entries by on every write (~30 days at 5s/ledger).
const TTL_EXTEND_TO: u32 = 518_400;
/// Extend once the entry's remaining TTL drops below this threshold.
const TTL_THRESHOLD: u32 = 100_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingGrant {
    pub grant_id: u64,
    pub org_id: u64,
    pub employee: Address,
    pub token: Address,
    pub total_amount: i128,
    pub claimed_amount: i128,
    pub start_at: u64,
    pub cliff_seconds: u64,
    pub vesting_seconds: u64,
    pub revoked: bool,
    pub revoked_at: u64,
}

#[contracttype]
pub enum DataKey {
    OrgRegistry,
    NextGrantId,
    Grant(u64),                   // grant_id -> VestingGrant
    EmployeeGrants(u64, Address), // (org_id, employee) -> Vec<u64>
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VestingError {
    GrantNotFound = 1,
    NotAuthorized = 2,
    GrantRevoked = 3,
    NothingClaimable = 4,
}

/// Minimal view of the Org Registry contract used to authorize grant
/// creation and revocation. Cross-contract client generated from this
/// trait — see the `org-registry` crate for the authoritative implementation.
#[contractclient(name = "OrgRegistryClient")]
pub trait OrgRegistryInterface {
    fn is_admin(env: Env, org_id: u64, address: Address) -> bool;
}

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    pub fn __constructor(env: Env, org_registry: Address) {
        env.storage()
            .instance()
            .set(&DataKey::OrgRegistry, &org_registry);
    }

    /// Transfers tokens from admin to contract. Creates grant.
    pub fn create_grant(
        env: Env,
        org_id: u64,
        employee: Address,
        token: Address,
        amount: i128,
        cliff_seconds: u64,
        vesting_seconds: u64,
        caller: Address,
    ) -> Result<u64, VestingError> {
        caller.require_auth();
        let is_admin =
            OrgRegistryClient::new(&env, &Self::org_registry(&env)).is_admin(&org_id, &caller);
        if !is_admin {
            return Err(VestingError::NotAuthorized);
        }

        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&caller, &env.current_contract_address(), &amount);

        let grant_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextGrantId)
            .unwrap_or(0)
            + 1;
        env.storage()
            .instance()
            .set(&DataKey::NextGrantId, &grant_id);

        let grant = VestingGrant {
            grant_id,
            org_id,
            employee: employee.clone(),
            token,
            total_amount: amount,
            claimed_amount: 0,
            start_at: env.ledger().timestamp(),
            cliff_seconds,
            vesting_seconds,
            revoked: false,
            revoked_at: 0,
        };
        Self::write_grant(&env, &grant);

        let index_key = DataKey::EmployeeGrants(org_id, employee);
        let mut grants: Vec<u64> = env
            .storage()
            .persistent()
            .get(&index_key)
            .unwrap_or_else(|| Vec::new(&env));
        grants.push_back(grant_id);
        env.storage().persistent().set(&index_key, &grants);
        env.storage()
            .persistent()
            .extend_ttl(&index_key, TTL_THRESHOLD, TTL_EXTEND_TO);

        Ok(grant_id)
    }

    /// Calculates vested amount minus already claimed. Transfers claimable
    /// tokens to employee.
    ///
    /// Vesting math: 0 before cliff; at/after cliff, linear from grant start
    /// to `vesting_seconds`. See ARCHITECTURE.md, "How Vesting Works".
    pub fn claim(env: Env, grant_id: u64, caller: Address) -> Result<i128, VestingError> {
        caller.require_auth();

        let mut grant = Self::read_grant(&env, grant_id)?;
        if caller != grant.employee {
            return Err(VestingError::NotAuthorized);
        }

        let now = env.ledger().timestamp();
        let vested = Self::vested_amount(&grant, now);
        let claimable = vested - grant.claimed_amount;
        if claimable <= 0 {
            return Err(VestingError::NothingClaimable);
        }

        let token_client = token::Client::new(&env, &grant.token);
        token_client.transfer(&env.current_contract_address(), &grant.employee, &claimable);

        grant.claimed_amount += claimable;
        Self::write_grant(&env, &grant);

        Ok(claimable)
    }

    /// Stops further vesting. Employee keeps what's already vested. Unvested
    /// tokens returned to admin.
    pub fn revoke_grant(env: Env, grant_id: u64, caller: Address) -> Result<(), VestingError> {
        caller.require_auth();

        let mut grant = Self::read_grant(&env, grant_id)?;
        if grant.revoked {
            return Err(VestingError::GrantRevoked);
        }
        let is_admin = OrgRegistryClient::new(&env, &Self::org_registry(&env))
            .is_admin(&grant.org_id, &caller);
        if !is_admin {
            return Err(VestingError::NotAuthorized);
        }

        let now = env.ledger().timestamp();
        let vested = Self::vested_amount(&grant, now);
        let unvested = grant.total_amount - vested;

        grant.revoked = true;
        grant.revoked_at = now;
        Self::write_grant(&env, &grant);

        if unvested > 0 {
            let token_client = token::Client::new(&env, &grant.token);
            token_client.transfer(&env.current_contract_address(), &caller, &unvested);
        }

        Ok(())
    }

    /// Read-only.
    pub fn get_grant(env: Env, grant_id: u64) -> Result<VestingGrant, VestingError> {
        Self::read_grant(&env, grant_id)
    }

    /// Read-only. Current claimable amount.
    pub fn get_claimable(env: Env, grant_id: u64) -> Result<i128, VestingError> {
        let grant = Self::read_grant(&env, grant_id)?;
        let now = env.ledger().timestamp();
        Ok(Self::vested_amount(&grant, now) - grant.claimed_amount)
    }

    /// Read-only.
    pub fn get_grants_by_employee(
        env: Env,
        org_id: u64,
        employee: Address,
    ) -> Result<Vec<VestingGrant>, VestingError> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::EmployeeGrants(org_id, employee))
            .unwrap_or_else(|| Vec::new(&env));

        let mut grants = Vec::new(&env);
        for id in ids.iter() {
            if let Some(grant) = env
                .storage()
                .persistent()
                .get::<_, VestingGrant>(&DataKey::Grant(id))
            {
                grants.push_back(grant);
            }
        }
        Ok(grants)
    }

    fn vested_amount(grant: &VestingGrant, now: u64) -> i128 {
        let effective_now = if grant.revoked { grant.revoked_at } else { now };

        if effective_now < grant.start_at + grant.cliff_seconds {
            return 0;
        }

        let elapsed = effective_now - grant.start_at;
        if elapsed >= grant.vesting_seconds {
            return grant.total_amount;
        }

        grant.total_amount * (elapsed as i128) / (grant.vesting_seconds as i128)
    }

    fn org_registry(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::OrgRegistry).unwrap()
    }

    fn read_grant(env: &Env, grant_id: u64) -> Result<VestingGrant, VestingError> {
        env.storage()
            .persistent()
            .get(&DataKey::Grant(grant_id))
            .ok_or(VestingError::GrantNotFound)
    }

    fn write_grant(env: &Env, grant: &VestingGrant) {
        let key = DataKey::Grant(grant.grant_id);
        env.storage().persistent().set(&key, grant);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

mod test;
