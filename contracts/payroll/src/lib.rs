#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, token, Address, Env, Vec,
};

/// Ledgers to extend persistent entries by on every write (~30 days at 5s/ledger).
const TTL_EXTEND_TO: u32 = 518_400;
/// Extend once the entry's remaining TTL drops below this threshold.
const TTL_THRESHOLD: u32 = 100_000;
/// Split percentages are basis points; must sum to this value.
const BPS_DENOMINATOR: u32 = 10_000;

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

impl PayFrequency {
    fn period_seconds(&self) -> u64 {
        match self {
            PayFrequency::Weekly => 7 * 86_400,
            PayFrequency::Biweekly => 14 * 86_400,
            PayFrequency::Monthly => 30 * 86_400,
        }
    }
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
    /// Snapshot of employees eligible at `prepare_batch` time.
    pub employees: Vec<Address>,
}

#[contracttype]
pub enum DataKey {
    OrgRegistry,
    Token,
    Config(u64),
    Schedule(u64, Address),
    Batch(u64, u64),
    ScheduleList(u64), // org_id -> Vec<Address>, employees with an active schedule
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
    NoEligiblePayments = 8,
    InsufficientApprovals = 9,
}

/// Minimal view of the Org Registry contract used to authorize privileged
/// actions. Cross-contract client generated from this trait — see the
/// `org-registry` crate for the authoritative implementation.
#[contractclient(name = "OrgRegistryClient")]
pub trait OrgRegistryInterface {
    fn is_admin(env: Env, org_id: u64, address: Address) -> bool;
    fn is_signer(env: Env, org_id: u64, address: Address) -> bool;
}

#[contract]
pub struct PayrollContract;

#[contractimpl]
impl PayrollContract {
    pub fn __constructor(env: Env, org_registry: Address, token: Address) {
        env.storage().instance().set(&DataKey::OrgRegistry, &org_registry);
        env.storage().instance().set(&DataKey::Token, &token);
    }

    /// Transfers USDC from caller to contract. Increases org's on-chain balance.
    pub fn fund_payroll(env: Env, org_id: u64, funder: Address, amount: i128) -> Result<(), PayrollError> {
        funder.require_auth();

        let token_client = token::Client::new(&env, &Self::token(&env));
        token_client.transfer(&funder, &env.current_contract_address(), &amount);

        let mut config = Self::read_or_init_config(&env, org_id);
        config.usdc_balance += amount;
        Self::write_config(&env, &config);
        Ok(())
    }

    /// Sets the number of signer approvals required to execute a payroll
    /// batch. 1 means no multi-sig.
    pub fn set_approval_threshold(
        env: Env,
        org_id: u64,
        threshold: u32,
        caller: Address,
    ) -> Result<(), PayrollError> {
        caller.require_auth();
        Self::require_admin(&env, org_id, &caller)?;

        let mut config = Self::read_or_init_config(&env, org_id);
        config.approval_threshold = if threshold == 0 { 1 } else { threshold };
        Self::write_config(&env, &config);
        Ok(())
    }

    /// Creates payment schedule for an employee. Validates splits sum to 100%.
    pub fn add_schedule(
        env: Env,
        org_id: u64,
        employee: Address,
        amount: i128,
        frequency: PayFrequency,
        splits: Vec<PaySplit>,
        caller: Address,
    ) -> Result<(), PayrollError> {
        caller.require_auth();
        Self::require_admin(&env, org_id, &caller)?;
        Self::validate_splits(&splits)?;

        let key = DataKey::Schedule(org_id, employee.clone());
        let is_new = !env.storage().persistent().has(&key);

        let schedule = PaymentSchedule {
            employee: employee.clone(),
            amount,
            frequency,
            next_payment_at: env.ledger().timestamp(),
            splits,
            active: true,
            total_paid: 0,
            last_paid_at: 0,
        };
        Self::write_schedule(&env, org_id, &schedule);

        if is_new {
            let list_key = DataKey::ScheduleList(org_id);
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
        }

        Ok(())
    }

    /// Modifies an existing schedule. Takes effect next pay cycle.
    pub fn update_schedule(
        env: Env,
        org_id: u64,
        employee: Address,
        amount: i128,
        frequency: PayFrequency,
        splits: Vec<PaySplit>,
        caller: Address,
    ) -> Result<(), PayrollError> {
        caller.require_auth();
        Self::require_admin(&env, org_id, &caller)?;
        Self::validate_splits(&splits)?;

        let mut schedule = Self::read_schedule(&env, org_id, &employee)?;
        schedule.amount = amount;
        schedule.frequency = frequency;
        schedule.splits = splits;
        Self::write_schedule(&env, org_id, &schedule);
        Ok(())
    }

    /// Deactivates schedule. Employee stops receiving payments. Callable by
    /// the org admin directly, or by the Org Registry contract as part of
    /// its employee-removal cascade.
    pub fn remove_schedule(
        env: Env,
        org_id: u64,
        employee: Address,
        caller: Address,
    ) -> Result<(), PayrollError> {
        let org_registry = Self::org_registry(&env);
        if caller == org_registry {
            org_registry.require_auth();
        } else {
            caller.require_auth();
            Self::require_admin(&env, org_id, &caller)?;
        }

        let mut schedule = Self::read_schedule(&env, org_id, &employee)?;
        schedule.active = false;
        Self::write_schedule(&env, org_id, &schedule);

        let list_key = DataKey::ScheduleList(org_id);
        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or_else(|| Vec::new(&env));
        if let Some(index) = list.iter().position(|a| a == employee) {
            list.remove(index as u32);
            env.storage().persistent().set(&list_key, &list);
        }

        Ok(())
    }

    /// Scans all active schedules where `next_payment_at <= now`. Creates a
    /// batch with total amount and employee count. Returns batch_id.
    pub fn prepare_batch(env: Env, org_id: u64) -> Result<u64, PayrollError> {
        let mut config = Self::read_or_init_config(&env, org_id);
        let now = env.ledger().timestamp();

        let schedule_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::ScheduleList(org_id))
            .unwrap_or_else(|| Vec::new(&env));

        let mut eligible: Vec<Address> = Vec::new(&env);
        let mut total_amount: i128 = 0;
        for employee in schedule_list.iter() {
            if let Ok(schedule) = Self::read_schedule(&env, org_id, &employee) {
                if schedule.active && schedule.next_payment_at <= now {
                    total_amount += schedule.amount;
                    eligible.push_back(employee);
                }
            }
        }

        if eligible.is_empty() {
            return Err(PayrollError::NoEligiblePayments);
        }

        let batch_id = config.current_batch_id + 1;
        let batch = BatchApproval {
            batch_id,
            total_amount,
            employee_count: eligible.len(),
            approvals: Vec::new(&env),
            executed: false,
            created_at: now,
            employees: eligible,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Batch(org_id, batch_id), &batch);
        env.storage().persistent().extend_ttl(
            &DataKey::Batch(org_id, batch_id),
            TTL_THRESHOLD,
            TTL_EXTEND_TO,
        );

        config.current_batch_id = batch_id;
        Self::write_config(&env, &config);

        Ok(batch_id)
    }

    /// Records signer approval. If threshold met, auto-executes.
    pub fn approve_batch(env: Env, org_id: u64, batch_id: u64, signer: Address) -> Result<(), PayrollError> {
        signer.require_auth();
        let is_signer = OrgRegistryClient::new(&env, &Self::org_registry(&env))
            .is_signer(&org_id, &signer);
        if !is_signer {
            return Err(PayrollError::NotAuthorized);
        }

        let mut batch = Self::read_batch(&env, org_id, batch_id)?;
        if batch.executed {
            return Err(PayrollError::AlreadyExecuted);
        }
        if !batch.approvals.contains(&signer) {
            batch.approvals.push_back(signer);
        }
        Self::write_batch(&env, org_id, &batch);

        let config = Self::read_or_init_config(&env, org_id);
        if batch.approvals.len() >= config.approval_threshold {
            Self::execute_payroll(env, org_id, batch_id)?;
        }

        Ok(())
    }

    /// For each eligible employee: transfers USDC per split config, updates
    /// `next_payment_at` and `total_paid`. Atomic — all employees in the
    /// batch are paid or none are (a single failed transfer aborts the whole
    /// call, reverting all storage writes made so far in this invocation).
    pub fn execute_payroll(env: Env, org_id: u64, batch_id: u64) -> Result<(), PayrollError> {
        let mut config = Self::read_or_init_config(&env, org_id);
        let mut batch = Self::read_batch(&env, org_id, batch_id)?;

        if batch.executed {
            return Err(PayrollError::AlreadyExecuted);
        }
        if config.approval_threshold > 1 && batch.approvals.len() < config.approval_threshold {
            return Err(PayrollError::InsufficientApprovals);
        }
        if config.usdc_balance < batch.total_amount {
            return Err(PayrollError::InsufficientBalance);
        }

        let token_client = token::Client::new(&env, &Self::token(&env));
        let contract_address = env.current_contract_address();
        let now = env.ledger().timestamp();
        let mut total_disbursed: i128 = 0;

        for employee in batch.employees.iter() {
            let mut schedule = Self::read_schedule(&env, org_id, &employee)?;
            if !schedule.active {
                continue;
            }

            for split in schedule.splits.iter() {
                let split_amount = schedule.amount * (split.percentage as i128) / (BPS_DENOMINATOR as i128);
                if split_amount > 0 {
                    token_client.transfer(&contract_address, &split.destination, &split_amount);
                }
            }

            total_disbursed += schedule.amount;
            schedule.total_paid += schedule.amount;
            schedule.last_paid_at = now;
            schedule.next_payment_at = now + schedule.frequency.period_seconds();
            Self::write_schedule(&env, org_id, &schedule);
        }

        config.usdc_balance -= total_disbursed;
        Self::write_config(&env, &config);

        batch.executed = true;
        Self::write_batch(&env, org_id, &batch);

        Ok(())
    }

    /// Withdraws unfunded USDC back to admin wallet. Cannot withdraw below
    /// amount needed for next pay cycle.
    pub fn withdraw_funds(env: Env, org_id: u64, amount: i128, caller: Address) -> Result<(), PayrollError> {
        caller.require_auth();
        Self::require_admin(&env, org_id, &caller)?;

        let mut config = Self::read_or_init_config(&env, org_id);
        let next_cycle_obligation = Self::next_cycle_obligation(&env, org_id);

        if config.usdc_balance - amount < next_cycle_obligation {
            return Err(PayrollError::InsufficientBalance);
        }

        let token_client = token::Client::new(&env, &Self::token(&env));
        token_client.transfer(&env.current_contract_address(), &caller, &amount);

        config.usdc_balance -= amount;
        Self::write_config(&env, &config);
        Ok(())
    }

    /// Employee can update their own split configuration.
    pub fn update_splits(
        env: Env,
        org_id: u64,
        employee: Address,
        splits: Vec<PaySplit>,
    ) -> Result<(), PayrollError> {
        employee.require_auth();
        Self::validate_splits(&splits)?;

        let mut schedule = Self::read_schedule(&env, org_id, &employee)?;
        schedule.splits = splits;
        Self::write_schedule(&env, org_id, &schedule);
        Ok(())
    }

    /// Read-only.
    pub fn get_schedule(env: Env, org_id: u64, employee: Address) -> Result<PaymentSchedule, PayrollError> {
        Self::read_schedule(&env, org_id, &employee)
    }

    /// Read-only.
    pub fn get_batch(env: Env, org_id: u64, batch_id: u64) -> Result<BatchApproval, PayrollError> {
        Self::read_batch(&env, org_id, batch_id)
    }

    /// Read-only. Current funded balance.
    pub fn get_org_balance(env: Env, org_id: u64) -> Result<i128, PayrollError> {
        Ok(Self::read_or_init_config(&env, org_id).usdc_balance)
    }

    fn require_admin(env: &Env, org_id: u64, caller: &Address) -> Result<(), PayrollError> {
        let is_admin = OrgRegistryClient::new(env, &Self::org_registry(env))
            .is_admin(&org_id, caller);
        if is_admin {
            Ok(())
        } else {
            Err(PayrollError::NotAuthorized)
        }
    }

    fn validate_splits(splits: &Vec<PaySplit>) -> Result<(), PayrollError> {
        if splits.is_empty() {
            return Err(PayrollError::InvalidSplits);
        }
        let total: u32 = splits.iter().map(|s| s.percentage).sum();
        if total != BPS_DENOMINATOR {
            return Err(PayrollError::InvalidSplits);
        }
        Ok(())
    }

    fn next_cycle_obligation(env: &Env, org_id: u64) -> i128 {
        let schedule_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::ScheduleList(org_id))
            .unwrap_or_else(|| Vec::new(env));

        let mut total: i128 = 0;
        for employee in schedule_list.iter() {
            if let Ok(schedule) = Self::read_schedule(env, org_id, &employee) {
                if schedule.active {
                    total += schedule.amount;
                }
            }
        }
        total
    }

    fn org_registry(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::OrgRegistry).unwrap()
    }

    fn token(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Token).unwrap()
    }

    fn read_or_init_config(env: &Env, org_id: u64) -> PayrollConfig {
        env.storage()
            .persistent()
            .get(&DataKey::Config(org_id))
            .unwrap_or_else(|| PayrollConfig {
                org_id,
                usdc_balance: 0,
                approval_threshold: 1,
                current_batch_id: 0,
                created_at: env.ledger().timestamp(),
            })
    }

    fn write_config(env: &Env, config: &PayrollConfig) {
        let key = DataKey::Config(config.org_id);
        env.storage().persistent().set(&key, config);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn read_schedule(env: &Env, org_id: u64, employee: &Address) -> Result<PaymentSchedule, PayrollError> {
        env.storage()
            .persistent()
            .get(&DataKey::Schedule(org_id, employee.clone()))
            .ok_or(PayrollError::ScheduleNotFound)
    }

    fn write_schedule(env: &Env, org_id: u64, schedule: &PaymentSchedule) {
        let key = DataKey::Schedule(org_id, schedule.employee.clone());
        env.storage().persistent().set(&key, schedule);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn read_batch(env: &Env, org_id: u64, batch_id: u64) -> Result<BatchApproval, PayrollError> {
        env.storage()
            .persistent()
            .get(&DataKey::Batch(org_id, batch_id))
            .ok_or(PayrollError::BatchNotFound)
    }

    fn write_batch(env: &Env, org_id: u64, batch: &BatchApproval) {
        let key = DataKey::Batch(org_id, batch.batch_id);
        env.storage().persistent().set(&key, batch);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

mod test;
