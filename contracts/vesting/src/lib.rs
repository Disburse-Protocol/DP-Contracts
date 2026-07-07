#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

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
    NextGrantId,
    Grant(u64), // grant_id -> VestingGrant
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

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    /// Transfers tokens from admin to contract. Creates grant.
    pub fn create_grant(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _token: Address,
        _amount: i128,
        _cliff_seconds: u64,
        _vesting_seconds: u64,
    ) -> Result<u64, VestingError> {
        unimplemented!("Wave task: create_grant — see contracts repo issue tracker")
    }

    /// Calculates vested amount minus already claimed. Transfers claimable
    /// tokens to employee.
    ///
    /// Vesting math: 0 before cliff; at/after cliff, linear from cliff to
    /// vesting_seconds. See ARCHITECTURE.md, "How Vesting Works".
    pub fn claim(_env: Env, _grant_id: u64) -> Result<i128, VestingError> {
        unimplemented!("Wave task: claim — see contracts repo issue tracker")
    }

    /// Stops further vesting. Employee keeps what's already vested. Unvested
    /// tokens returned to admin.
    pub fn revoke_grant(_env: Env, _grant_id: u64) -> Result<(), VestingError> {
        unimplemented!("Wave task: revoke_grant — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_grant(_env: Env, _grant_id: u64) -> Result<VestingGrant, VestingError> {
        unimplemented!("Wave task: get_grant — see contracts repo issue tracker")
    }

    /// Read-only. Current claimable amount.
    pub fn get_claimable(_env: Env, _grant_id: u64) -> Result<i128, VestingError> {
        unimplemented!("Wave task: get_claimable — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_grants_by_employee(
        _env: Env,
        _org_id: u64,
        _employee: Address,
    ) -> Result<Vec<VestingGrant>, VestingError> {
        unimplemented!("Wave task: get_grants_by_employee — see contracts repo issue tracker")
    }
}

mod test;
