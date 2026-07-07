#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

/// Per-organization payroll config (keyed by org_id from Org Registry).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayrollConfig {
    pub org_id: u64,
    pub usdc_balance: i128,
    pub approval_threshold: u32,
    pub current_batch_id: u64,
    pub created_at: u64,
}

/// Per-employee payment schedule (keyed by org_id + employee Address).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentSchedule {
    pub employee: Address,
    pub amount: i128,
    pub frequency: PayFrequency,
    pub next_payment_at: u64,
    pub splits: Vec<PaySplit>,
    pub active: bool,
    pub total_paid: i128,
    pub last_paid_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaySplit {
    pub destination: Address,
    /// Basis points. 7000 = 70%.
    pub percentage: u32,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayFrequency {
    Weekly,
    Biweekly,
    Monthly,
}

/// Per-batch approval tracking (keyed by org_id + batch_id).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchApproval {
    pub batch_id: u64,
    pub total_amount: i128,
    pub employee_count: u32,
    pub approvals: Vec<Address>,
    pub executed: bool,
    pub created_at: u64,
}

#[contracttype]
pub enum DataKey {
    Config(u64),            // org_id -> PayrollConfig
    Schedule(u64, Address), // (org_id, employee) -> PaymentSchedule
    Batch(u64, u64),        // (org_id, batch_id) -> BatchApproval
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PayrollError {
    OrgNotFound = 1,
    ScheduleNotFound = 2,
    BatchNotFound = 3,
    NotAuthorized = 4,
    InsufficientBalance = 5,
    InvalidSplits = 6,
    AlreadyExecuted = 7,
}

#[contract]
pub struct PayrollContract;

#[contractimpl]
impl PayrollContract {
    /// Transfers USDC from caller to contract. Increases org's on-chain balance.
    ///
    /// See ARCHITECTURE.md, "Payroll Contract" interface table.
    pub fn fund_payroll(_env: Env, _org_id: u64, _amount: i128) -> Result<(), PayrollError> {
        unimplemented!("Wave task: fund_payroll — see contracts repo issue tracker")
    }

    /// Creates payment schedule for an employee. Validates splits sum to 100%.
    pub fn add_schedule(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _amount: i128,
        _frequency: PayFrequency,
        _splits: Vec<PaySplit>,
    ) -> Result<(), PayrollError> {
        unimplemented!("Wave task: add_schedule — see contracts repo issue tracker")
    }

    /// Modifies an existing schedule. Takes effect next pay cycle.
    pub fn update_schedule(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _amount: i128,
        _frequency: PayFrequency,
        _splits: Vec<PaySplit>,
    ) -> Result<(), PayrollError> {
        unimplemented!("Wave task: update_schedule — see contracts repo issue tracker")
    }

    /// Deactivates schedule. Employee stops receiving payments.
    pub fn remove_schedule(
        _env: Env,
        _org_id: u64,
        _employee: Address,
    ) -> Result<(), PayrollError> {
        unimplemented!("Wave task: remove_schedule — see contracts repo issue tracker")
    }

    /// Scans all active schedules where next_payment_at <= now. Creates a batch
    /// with total amount and employee count. Returns batch_id.
    pub fn prepare_batch(_env: Env, _org_id: u64) -> Result<u64, PayrollError> {
        unimplemented!("Wave task: prepare_batch — see contracts repo issue tracker")
    }

    /// Records signer approval. If threshold met, auto-executes.
    pub fn approve_batch(_env: Env, _org_id: u64, _batch_id: u64) -> Result<(), PayrollError> {
        unimplemented!("Wave task: approve_batch — see contracts repo issue tracker")
    }

    /// For each eligible employee: transfers USDC per split config, updates
    /// next_payment_at and total_paid. Fails if org balance insufficient.
    /// Must be atomic — all employees in the batch are paid or none are.
    pub fn execute_payroll(_env: Env, _org_id: u64, _batch_id: u64) -> Result<(), PayrollError> {
        unimplemented!("Wave task: execute_payroll — see contracts repo issue tracker")
    }

    /// Withdraws unfunded USDC back to admin wallet. Cannot withdraw below
    /// amount needed for next pay cycle.
    pub fn withdraw_funds(_env: Env, _org_id: u64, _amount: i128) -> Result<(), PayrollError> {
        unimplemented!("Wave task: withdraw_funds — see contracts repo issue tracker")
    }

    /// Employee can update their own split configuration.
    pub fn update_splits(
        _env: Env,
        _org_id: u64,
        _employee: Address,
        _splits: Vec<PaySplit>,
    ) -> Result<(), PayrollError> {
        unimplemented!("Wave task: update_splits — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_schedule(
        _env: Env,
        _org_id: u64,
        _employee: Address,
    ) -> Result<PaymentSchedule, PayrollError> {
        unimplemented!("Wave task: get_schedule — see contracts repo issue tracker")
    }

    /// Read-only.
    pub fn get_batch(
        _env: Env,
        _org_id: u64,
        _batch_id: u64,
    ) -> Result<BatchApproval, PayrollError> {
        unimplemented!("Wave task: get_batch — see contracts repo issue tracker")
    }

    /// Read-only. Current funded balance.
    pub fn get_org_balance(_env: Env, _org_id: u64) -> Result<i128, PayrollError> {
        unimplemented!("Wave task: get_org_balance — see contracts repo issue tracker")
    }
}

mod test;
