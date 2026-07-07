# Disburse Protocol — Architecture

Decentralized payroll protocol on Stellar. Employers fund payroll contracts with USDC, employees get paid automatically on schedule with verifiable on-chain disbursement. All payroll logic and fund custody lives on-chain in Soroban smart contracts. The backend is a read-only chain indexer. The frontend is the admin and employee interface.

---

## Organization Structure

```
github.com/Disburse-Protocol/
├── contracts        # Soroban smart contracts (Rust)
├── backend          # Chain indexer and query API (Node/Express/TypeScript)
└── frontend         # Employer dashboard + Employee portal (Next.js/TypeScript)
```

Three repos. The contracts repo is the protocol — payroll schedules, fund custody, disbursement logic, and payment history all live on-chain. The backend indexes contract events into a query-optimized format and serves them over REST and WebSocket. The frontend is the primary client, reading from the backend for speed and writing directly to contracts via Stellar SDK.

---

## System Overview

```
┌────────────────────────────────────────────────────────────────┐
│                          Frontend                              │
│                Next.js 14 · TypeScript · Tailwind               │
│                                                                 │
│   Employer Dashboard ── Employee Portal ── Org Settings         │
│          │                       │                  │          │
│     ┌────┴───────────────────────┴──────────────────┘          │
│     │                                                           │
│     │  READS ──────── Backend API (indexed queries)             │
│     │  WRITES ─────── Stellar Network (signed transactions)     │
│     │  FALLBACK ───── Soroban RPC (if backend unavailable)       │
│     │                                                           │
└─────┼────────────────────────────────────────────────────────────┘
      │
      │  READS (REST + WebSocket)
      ▼
┌────────────────────────────────────────────────────────────────┐
│                          Backend                                │
│               Express · TypeScript · Indexer                    │
│                                                                 │
│   Event Watcher ──── Query API ──── WebSocket Server            │
│        │                                                        │
│        │  Subscribes to Soroban contract events                 │
│        │  Indexes into lightweight store                        │
│        │  Serves filtered, sorted, paginated results             │
│        │                                                        │
│        │  • READ-ONLY — never writes to contracts                │
│        │  • NOT source of truth — contracts are                  │
│        │  • Fully rebuildable from chain at any time              │
│                                                                 │
└───────────────────────────┬──────────────────────────────────────┘
                            │
                            │  Soroban RPC (event stream + contract reads)
                            ▼
┌────────────────────────────────────────────────────────────────┐
│                      Stellar Network                            │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │   Payroll    │  │  Org         │  │  Vesting            │   │
│  │   Contract   │  │  Registry    │  │  Contract           │   │
│  │              │  │              │  │                     │   │
│  │ Schedules    │  │ Orgs         │  │ Token vesting       │   │
│  │ Disbursement │  │ Employees    │  │ Cliff + linear      │   │
│  │ Splits       │  │ Roles        │  │ Claimable balance   │   │
│  │ Multi-sig    │  │ Signers      │  │                     │   │
│  │ Fund custody │  │              │  │                     │   │
│  └──────────────┘  └──────────────┘  └─────────────────────┘   │
│                                                                 │
│  ┌──────────────┐                                               │
│  │    USDC      │                                               │
│  │ (Stellar     │                                               │
│  │  Asset)      │                                               │
│  └──────────────┘                                               │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

---

## Repo: `contracts`

Three Soroban contracts.

### 1. Payroll Contract

Core contract. Holds USDC, stores payment schedules, and executes disbursements.

**How Payroll Execution Works**

The employer funds the contract with USDC. Each employee has a payment schedule stored on-chain — amount, frequency, next payment date, and optional split configuration. When the next payment date arrives, anyone can call `execute_payroll` to trigger disbursement for all eligible employees in a single transaction. The contract checks each employee's schedule, transfers their USDC, and updates the next payment date.

"Anyone can call" is intentional — the employer can set up a cron job, a Stellar automation bot, or even the employees themselves can trigger it. The contract doesn't care who calls it, only whether the payment date has arrived. No trust required.

**Payment Splits**

An employee can configure their pay to auto-split across multiple Stellar addresses:

```
Employee total: 2,000 USDC/month
  → 1,400 USDC (70%) → main wallet
  →   400 USDC (20%) → savings wallet
  →   200 USDC (10%) → tax reserve wallet
```

The contract enforces splits atomically — all transfers happen in the same transaction or none do. Split percentages must sum to 100%.

**Multi-Signatory Approval**

For organizations that need oversight, payroll execution can require multiple signers. The org admin configures a threshold (e.g. 2-of-3). Each signer submits an approval transaction. The contract only releases funds once the threshold is met.

```
Approval flow:
  Signer A approves payroll batch #7 → stored on-chain, not yet executed
  Signer B approves payroll batch #7 → threshold met, payroll executes
```

Approvals are per pay cycle. Each cycle generates a batch ID. Signers approve the batch, not individual payments.

**Storage**

```rust
// Per-organization payroll config (keyed by org_id from Org Registry)
pub struct PayrollConfig {
    pub org_id:             u64,
    pub usdc_balance:       i128,          // funded balance held by contract
    pub approval_threshold: u32,           // 1 = no multi-sig, 2+ = multi-sig
    pub current_batch_id:   u64,
    pub created_at:         u64,
}

// Per-employee payment schedule (keyed by org_id + employee Address)
pub struct PaymentSchedule {
    pub employee:           Address,
    pub amount:             i128,          // total USDC per period
    pub frequency:          PayFrequency,
    pub next_payment_at:    u64,           // ledger timestamp
    pub splits:             Vec<PaySplit>,
    pub active:             bool,
    pub total_paid:         i128,          // lifetime disbursed
    pub last_paid_at:       u64,
}

pub struct PaySplit {
    pub destination:        Address,
    pub percentage:         u32,           // basis points (7000 = 70%)
}

pub enum PayFrequency {
    Weekly,
    Biweekly,
    Monthly,
}

// Per-batch approval tracking (keyed by org_id + batch_id)
pub struct BatchApproval {
    pub batch_id:           u64,
    pub total_amount:       i128,          // sum of all eligible payments
    pub employee_count:     u32,
    pub approvals:          Vec<Address>,  // signers who have approved
    pub executed:           bool,
    pub created_at:         u64,
}
```

**Interface**

| Function | Caller | What Happens |
|---|---|---|
| `fund_payroll(org_id, amount)` | Org admin/signer | Transfers USDC from caller to contract. Increases org's on-chain balance. |
| `add_schedule(org_id, employee, amount, frequency, splits)` | Org admin | Creates payment schedule for an employee. Validates splits sum to 100%. |
| `update_schedule(org_id, employee, amount, frequency, splits)` | Org admin | Modifies an existing schedule. Takes effect next pay cycle. |
| `remove_schedule(org_id, employee)` | Org admin | Deactivates schedule. Employee stops receiving payments. |
| `prepare_batch(org_id) → batch_id` | Anyone | Scans all active schedules where `next_payment_at <= now`. Creates a batch with total amount and employee count. Returns batch_id. |
| `approve_batch(org_id, batch_id)` | Org signer | Records signer approval. If threshold met, auto-executes. |
| `execute_payroll(org_id, batch_id)` | Anyone (if threshold=1) or auto (if multi-sig threshold met) | For each eligible employee: transfers USDC per split config, updates `next_payment_at` and `total_paid`. Fails if org balance insufficient. |
| `withdraw_funds(org_id, amount)` | Org admin | Withdraws unfunded USDC back to admin wallet. Cannot withdraw below amount needed for next pay cycle. |
| `update_splits(org_id, employee, splits)` | Employee | Employee can update their own split configuration. |
| `get_schedule(org_id, employee) → PaymentSchedule` | Anyone | Read-only. |
| `get_batch(org_id, batch_id) → BatchApproval` | Anyone | Read-only. |
| `get_org_balance(org_id) → i128` | Anyone | Read-only. Current funded balance. |

**Events Emitted**

```rust
PayrollFunded       { org_id, funder, amount, new_balance }
ScheduleAdded       { org_id, employee, amount, frequency }
ScheduleUpdated     { org_id, employee, amount, frequency }
ScheduleRemoved     { org_id, employee }
BatchPrepared       { org_id, batch_id, total_amount, employee_count }
BatchApproved       { org_id, batch_id, signer, approvals_so_far, threshold }
PayrollExecuted     { org_id, batch_id, total_disbursed, employee_count }
EmployeePaid        { org_id, employee, amount, splits: Vec<(Address, i128)> }
SplitsUpdated       { org_id, employee, splits }
FundsWithdrawn      { org_id, amount, remaining_balance }
```

---

### 2. Org Registry Contract

Manages organizations, employee rosters, and signer roles. Separates org management from payroll execution so each contract stays focused.

**Storage**

```rust
pub struct Organization {
    pub org_id:         u64,
    pub name:           Symbol,
    pub admin:          Address,        // can add/remove employees and signers
    pub signers:        Vec<Address>,   // can approve payroll batches
    pub employee_count: u32,
    pub created_at:     u64,
}

pub struct Employee {
    pub address:        Address,
    pub display_name:   Symbol,
    pub role:           Symbol,         // e.g. "engineer", "designer"
    pub added_at:       u64,
    pub active:         bool,
}

// Global
next_org_id: u64
```

**Interface**

| Function | Caller | What Happens |
|---|---|---|
| `create_org(name, admin)` | Anyone | Creates an organization. Caller becomes admin. Returns org_id. |
| `add_signer(org_id, signer)` | Org admin | Adds an address as a payroll approver. |
| `remove_signer(org_id, signer)` | Org admin | Removes a signer. Cannot remove last signer. |
| `add_employee(org_id, employee, display_name, role)` | Org admin | Registers an employee in the org. |
| `remove_employee(org_id, employee)` | Org admin | Deactivates employee. Triggers schedule removal in payroll contract. |
| `update_employee(org_id, employee, display_name, role)` | Org admin | Updates employee metadata. |
| `transfer_admin(org_id, new_admin)` | Org admin | Transfers admin role. |
| `get_org(org_id) → Organization` | Anyone | Read-only. |
| `get_employee(org_id, employee) → Employee` | Anyone | Read-only. |
| `get_employees(org_id) → Vec<Employee>` | Anyone | Read-only. All employees in org. |
| `is_signer(org_id, address) → bool` | Payroll contract | Used by payroll contract to validate batch approvals. |
| `is_admin(org_id, address) → bool` | Payroll contract | Used by payroll contract to validate admin actions. |

**Cross-Contract Calls**

The payroll contract calls the org registry to validate permissions:
- Before any admin action → `org_registry.is_admin(org_id, caller)`
- Before batch approval → `org_registry.is_signer(org_id, caller)`
- When removing an employee → org registry calls `payroll.remove_schedule(org_id, employee)`

---

### 3. Vesting Contract

Handles token compensation with cliff and linear vesting schedules. Separate from payroll because vesting has different mechanics — it's not recurring payments, it's a one-time grant that unlocks over time.

**How Vesting Works**

An employer creates a vesting grant for an employee: 10,000 USDC (or any Stellar asset) over 24 months with a 6-month cliff. The full amount is locked in the contract at grant creation. Nothing is claimable for the first 6 months. After the cliff, 25% (6/24) unlocks immediately. Then the remaining 75% unlocks linearly each month. The employee calls `claim` whenever they want to withdraw their unlocked tokens.

```
Grant: 10,000 USDC, 24-month vesting, 6-month cliff

Month 0-5:   claimable = 0
Month 6:     claimable = 2,500 (cliff unlock: 6/24 × 10,000)
Month 7:     claimable = 2,916 (7/24 × 10,000)
Month 12:    claimable = 5,000 (12/24 × 10,000)
Month 24:    claimable = 10,000 (fully vested)
```

**Storage**

```rust
pub struct VestingGrant {
    pub grant_id:           u64,
    pub org_id:             u64,
    pub employee:           Address,
    pub token:              Address,       // USDC or any Stellar asset
    pub total_amount:       i128,
    pub claimed_amount:     i128,
    pub start_at:           u64,           // ledger timestamp
    pub cliff_seconds:      u64,           // e.g. 6 months in seconds
    pub vesting_seconds:    u64,           // e.g. 24 months in seconds
    pub revoked:            bool,
    pub revoked_at:         u64,
}

// Global
next_grant_id: u64
```

**Interface**

| Function | Caller | What Happens |
|---|---|---|
| `create_grant(org_id, employee, token, amount, cliff_seconds, vesting_seconds)` | Org admin | Transfers tokens from admin to contract. Creates grant. |
| `claim(grant_id)` | Employee | Calculates vested amount minus already claimed. Transfers claimable tokens to employee. |
| `revoke_grant(grant_id)` | Org admin | Stops further vesting. Employee keeps what's already vested. Unvested tokens returned to admin. |
| `get_grant(grant_id) → VestingGrant` | Anyone | Read-only. |
| `get_claimable(grant_id) → i128` | Anyone | Read-only. Current claimable amount. |
| `get_grants_by_employee(org_id, employee) → Vec<VestingGrant>` | Anyone | Read-only. |

**Events Emitted**

```rust
GrantCreated   { grant_id, org_id, employee, token, amount, cliff_seconds, vesting_seconds }
TokensClaimed  { grant_id, employee, amount_claimed, total_claimed, remaining }
GrantRevoked   { grant_id, employee, vested_amount, returned_amount }
```

---

## Cross-Contract Interaction

```
┌──────────────┐         ┌──────────────┐
│   Payroll    │────────▶│     Org      │
│   Contract   │         │   Registry   │
│              │ validate│              │
│ Holds USDC   │────────▶│ is_admin?    │
│ Schedules    │         │ is_signer?   │
│ Batches      │         │              │
│ Disbursement │         │ Orgs         │
│              │         │ Employees    │
│              │◀────────│ Signers      │
│              │  remove │              │
│              │ schedule│              │
└──────────────┘         └──────┬───────┘
                                │
                                │ validates admin
                                │ for grant creation
                                ▼
                         ┌──────────────┐
                         │   Vesting    │
                         │   Contract   │
                         │              │
                         │ Grants       │
                         │ Cliff+linear │
                         │ Claims       │
                         └──────────────┘
```

- Payroll contract calls Org Registry to check `is_admin` and `is_signer` before executing privileged actions.
- Org Registry calls Payroll contract to remove an employee's schedule when the employee is removed from the org.
- Vesting contract calls Org Registry to validate admin permissions for grant creation and revocation.
- Payroll and Vesting contracts are independent — an employee can receive both recurring pay and vesting grants.

---

## Repo: `backend`

Read-only indexer. Watches Soroban events, builds query-optimized views, serves REST and WebSocket. Never writes to contracts. Fully rebuildable from chain.

### Why It Exists

Querying three contracts individually for every frontend page load is slow. The backend pre-joins payroll schedules with employee profiles, computes upcoming payment dates, aggregates org-level stats, and serves it all in single API responses with filtering and pagination.

### Event Watcher

```
Stellar Event Stream (Soroban RPC)
    │
    ▼
Event Watcher
    │
    ├─ PayrollFunded     → update org balance
    ├─ ScheduleAdded     → index employee schedule with profile
    ├─ ScheduleUpdated   → update indexed schedule
    ├─ ScheduleRemoved   → remove from active schedules
    ├─ BatchPrepared     → index pending batch
    ├─ BatchApproved     → update approval status
    ├─ PayrollExecuted   → mark batch complete, update payment history
    ├─ EmployeePaid      → index individual payment record
    ├─ GrantCreated      → index vesting grant
    ├─ TokensClaimed     → update grant claimed amount
    ├─ GrantRevoked      → mark grant revoked
    ├─ OrgCreated        → index new org
    ├─ EmployeeAdded     → index employee
    └─ EmployeeRemoved   → deactivate employee
```

### Indexed Data

```
Indexed Org = {
    org_id,
    name,
    admin,
    signers,
    employee_count,
    total_disbursed (computed from payment events),
    usdc_balance,
    next_payroll_date (earliest next_payment_at across schedules),
    runway_months (balance / monthly burn rate),
    created_at
}

Indexed Employee = {
    address,
    display_name,
    role,
    org_id,
    schedule: {
        amount,
        frequency,
        next_payment_at,
        splits,
        total_paid
    },
    vesting_grants: [{
        grant_id,
        total_amount,
        claimed_amount,
        claimable_now,
        cliff_date,
        fully_vested_date
    }],
    payment_history: [{
        batch_id,
        amount,
        splits,
        paid_at
    }]
}

Indexed Batch = {
    batch_id,
    org_id,
    total_amount,
    employee_count,
    approvals,
    threshold,
    executed,
    created_at
}
```

### Query API (REST)

```
Organizations
  POST /api/orgs                              → not a write — triggers reindex of a specific org
  GET  /api/orgs/:orgId                       → org detail with balance, stats, runway
  GET  /api/orgs/:orgId/employees             → all employees with schedules and grants
  GET  /api/orgs/:orgId/payroll/upcoming      → next batch preview (who gets paid, amounts)
  GET  /api/orgs/:orgId/payroll/history       → past batches with details
  GET  /api/orgs/:orgId/batches/:batchId      → single batch detail with approval status

Employee
  GET  /api/employee/:address                 → employee profile across all orgs
  GET  /api/employee/:address/payments        → payment history
  GET  /api/employee/:address/vesting         → all vesting grants with claimable amounts

Stats
  GET  /api/orgs/:orgId/stats                 → total disbursed, avg payment, runway, etc.
```

### WebSocket Server

```
ws://backend/ws?address=GXXX

Events pushed:
  payroll:funded        → org balance updated
  payroll:executed      → you got paid (employee) / batch completed (admin)
  batch:pending         → batch prepared, awaiting approval (signers)
  batch:approved        → signer approved, X of Y threshold
  vesting:claimable     → new tokens available to claim
  schedule:updated      → your pay schedule changed
```

### Rebuild

```bash
npm run rebuild -- --from-ledger 0
```

Replays all contract events from genesis and reconstructs the entire indexed store. Proves the backend is stateless.

---

## Repo: `frontend`

Next.js 14 with App Router. Two primary user experiences: employer dashboard and employee portal.

### Pages

**Employer Dashboard**

| Route | Purpose |
|---|---|
| `/` | Landing page — product overview |
| `/dashboard` | Org overview — balance, next payroll date, runway, employee count |
| `/dashboard/fund` | Fund payroll — deposit USDC into contract |
| `/dashboard/employees` | Employee roster — add, edit, remove, view schedules |
| `/dashboard/employees/[address]` | Single employee — schedule, payment history, vesting grants |
| `/dashboard/payroll` | Payroll management — prepare batch, approve, view history |
| `/dashboard/payroll/[batchId]` | Batch detail — approval status, employee breakdown |
| `/dashboard/vesting` | Vesting grants — create, view, revoke |
| `/dashboard/settings` | Org settings — signers, approval threshold, admin transfer |

**Employee Portal**

| Route | Purpose |
|---|---|
| `/portal` | Employee home — next payment date, recent payments, vesting summary |
| `/portal/payments` | Full payment history with split breakdown |
| `/portal/vesting` | Vesting grants — claimable amount, claim button, schedule visualization |
| `/portal/splits` | Manage payment splits — update destination wallets and percentages |

### Data Flow

```
READS:
  Primary  → Backend REST API (fast indexed queries)
  Fallback → Soroban RPC (direct contract reads if backend down)

WRITES:
  Always   → Stellar Network via Freighter (signed transactions)
  Never    → Backend

REAL-TIME:
  Primary  → Backend WebSocket
  Fallback → Soroban RPC event polling
```

### Write Path Examples

```
Employer funds payroll:
  → Frontend builds tx: payroll.fund_payroll(org_id, amount)
  → Freighter popup — employer signs
  → Submit to Stellar
  → Contract transfers USDC from employer wallet to contract
  → Event: PayrollFunded
  → Backend indexes → WebSocket → dashboard updates balance

Employer adds employee schedule:
  → Frontend builds tx: payroll.add_schedule(org_id, employee, amount, freq, splits)
  → Freighter → sign → submit
  → Contract stores schedule
  → Event: ScheduleAdded
  → Backend indexes → employee appears in roster

Payroll execution (single-sig org):
  → Anyone calls: payroll.prepare_batch(org_id)
  → Contract creates batch from eligible schedules
  → Event: BatchPrepared
  → Anyone calls: payroll.execute_payroll(org_id, batch_id)
  → Contract disburses USDC to all employees per their splits
  → Events: PayrollExecuted + EmployeePaid per employee
  → Backend indexes → all parties see updates

Payroll execution (multi-sig org):
  → Anyone calls: payroll.prepare_batch(org_id)
  → Signer A calls: payroll.approve_batch(org_id, batch_id)
  → Event: BatchApproved (1 of 2)
  → Signer B calls: payroll.approve_batch(org_id, batch_id)
  → Threshold met → contract auto-executes payroll
  → Events: BatchApproved (2 of 2) + PayrollExecuted + EmployeePaid

Employee claims vested tokens:
  → Frontend builds tx: vesting.claim(grant_id)
  → Freighter → sign → submit
  → Contract computes claimable, transfers to employee
  → Event: TokensClaimed
  → Backend indexes → vesting dashboard updates

Employee updates splits:
  → Frontend builds tx: payroll.update_splits(org_id, employee, new_splits)
  → Freighter → sign → submit
  → Contract validates splits sum to 100%, stores new config
  → Event: SplitsUpdated
  → Takes effect on next pay cycle
```

### Wallet Integration

Primary wallet: **Freighter** (Stellar browser extension).

All signing via Freighter. No private keys handled by the frontend. Wallet address determines whether the user sees the employer dashboard or employee portal (or both, if they're an admin who's also an employee somewhere).

### Fallback Mode

If the backend is down, the frontend reads directly from contracts:
- Org details → `org_registry.get_org(org_id)`
- Employee list → `org_registry.get_employees(org_id)`
- Schedules → `payroll.get_schedule(org_id, employee)` per employee
- Vesting → `vesting.get_grants_by_employee(org_id, employee)`

Slower, no pagination, no computed stats like runway — but functional.

---

## Complete Payroll Cycle

```
1. Setup (one-time)
   Admin creates org       → org_registry.create_org(name)
   Admin adds signers      → org_registry.add_signer(org_id, signer)
   Admin adds employees    → org_registry.add_employee(org_id, address, name, role)
   Admin creates schedules → payroll.add_schedule(org_id, employee, amount, freq, splits)
   Admin creates grants    → vesting.create_grant(org_id, employee, token, amount, cliff, duration)

2. Funding
   Admin deposits USDC     → payroll.fund_payroll(org_id, amount)
   Contract holds USDC, balance visible on-chain

3. Pay day
   Bot/admin/anyone        → payroll.prepare_batch(org_id)
   Contract scans schedules, creates batch of eligible employees

   If threshold == 1:
     Anyone               → payroll.execute_payroll(org_id, batch_id)
     USDC disbursed immediately

   If threshold > 1:
     Each signer           → payroll.approve_batch(org_id, batch_id)
     On threshold met      → auto-executes
     USDC disbursed to all employees per their split configs

4. Post-payment
   Each employee's next_payment_at advances by their frequency
   Payment history recorded on-chain via events
   Employee total_paid updated
   Org balance decremented

5. Ongoing
   Employees claim vested tokens anytime → vesting.claim(grant_id)
   Employees update splits anytime       → payroll.update_splits(org_id, employee, splits)
   Admin tops up balance as needed       → payroll.fund_payroll(org_id, amount)
```

---

## Security Considerations

**Contract level:**
- All admin actions validated via cross-contract call to `org_registry.is_admin`.
- All batch approvals validated via `org_registry.is_signer`.
- Employees can only update their own splits. Cannot modify amount, frequency, or other employees' data.
- `withdraw_funds` enforces minimum balance — admin cannot withdraw below next cycle's total obligations.
- Vesting revocation returns only unvested tokens. Already-vested tokens belong to the employee regardless.
- `execute_payroll` is atomic — all employees in the batch are paid or none are. Prevents partial execution.

**Backend level:**
- Read-only. No write endpoints. No secrets that affect fund safety.
- Rebuildable from chain. Compromising the backend cannot result in fund loss or schedule manipulation.

**Frontend level:**
- No private keys. All signing via Freighter.
- Writes bypass the backend. Admin actions go directly to Stellar.
- Role-based UI is a convenience — contract-level permission checks are the real enforcement.

**Protocol level:**
- Org admin is a single point of trust per organization. `transfer_admin` enables succession.
- Multi-sig approval adds oversight for larger orgs. Threshold is configurable.
- No global admin or protocol owner can access org funds. Each org's USDC is isolated in the contract's storage, keyed by org_id.

---

## Deployment

| Component | Target |
|---|---|
| Payroll Contract | Stellar Testnet → Mainnet |
| Org Registry | Stellar Testnet → Mainnet |
| Vesting Contract | Stellar Testnet → Mainnet |
| Backend | Railway / Render / VPS |
| Frontend | Vercel |

---

## Local Development

1. Install Soroban CLI and Rust toolchain.
2. Deploy all three contracts to Stellar Testnet.
3. Fund test accounts via Friendbot.
4. Set contract addresses in backend `.env`.
5. Start backend (`npm run dev`) — begins indexing from current ledger.
6. Set backend URL and contract addresses in frontend `.env.local`.
7. Start frontend (`npm run dev`).
8. Install Freighter browser extension, switch to Testnet.
9. Create a test org, add employees (use multiple Freighter accounts), fund payroll, run a batch.
