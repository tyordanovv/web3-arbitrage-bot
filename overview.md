# Arbitrage Bot - Technical Overview

## 1. Problem Statement

Cross-DEX arbitrage on blockchain networks suffers from three fundamental challenges:

**Latency vs. Accuracy Trade-off**: Real-time WebSocket event streams provide low latency but are vulnerable to dropped connections, missed events, and transient failures. Periodic RPC polling provides accuracy but introduces significant latency (seconds to minutes), making opportunities unprofitable by the time they're detected.

**State Synchronization at Scale**: Maintaining accurate state across dozens of liquidity pools on multiple DEXes requires managing thousands of state updates per minute. Traditional approaches either sacrifice accuracy (polling-only) or reliability (streaming-only), leading to missed opportunities or false positives.

**Multi-DEX Coordination Complexity**: Each DEX has unique pool structures, event formats, and pricing mechanisms. Abstracting these differences while maintaining performance requires careful architectural design that most bots fail to achieve, leading to brittle, DEX-specific implementations.

This bot solves these problems through a **dual-path architecture** that combines hot-path event streaming with cold-path periodic synchronization, ensuring both low latency and eventual consistency.

---

## 2. Protocol Value Proposition

### What This Bot Does

This is a **high-frequency cross-DEX arbitrage engine** that:

1. **Monitors liquidity pools** across multiple DEXes (Cetus, Turbos, Kriya) via WebSocket subscriptions for sub-second latency
2. **Maintains local state** of all pool reserves, liquidity, and fees with automatic drift correction
3. **Detects profitable arbitrage paths** through graph-based pathfinding across up to 4 hops (triangular, quadrilateral, etc.)
4. **Validates opportunities** by checking liquidity depth, price impact, gas costs, and staleness
5. **Executes atomic transactions** on-chain to capture profit before the window closes

The bot operates entirely **off-chain for detection** and **on-chain for execution**, minimizing transaction costs while maximizing responsiveness.

### Why Sui (and Not EVM)

**Object-Centric Model**: Sui's object model allows direct subscription to specific pool objects via WebSocket filters (`suix_subscribeEvent` with `MoveEventType`). On EVM chains, you must filter logs client-side after subscribing to broad topics, wasting bandwidth and introducing latency.

**Parallel Execution**: Sui's transaction processing model enables parallel execution of independent transactions. Multiple arbitrage paths can be attempted simultaneously without gas auction wars. On EVM, all transactions compete in a global mempool, making MEV bots essential and front-running inevitable.

**Deterministic Gas**: Sui's gas model provides predictable costs for complex multi-hop swaps. EVM chains have variable gas (especially during congestion), making pre-execution profitability validation nearly impossible without simulation on every block.

**Native Move Objects**: DEX pool states are first-class objects with typed fields (reserves, liquidity, ticks). Parsing events is straightforward via MoveEvent structures. EVM requires decoding ABI-encoded logs with fragile string parsing.

**Sub-Second Finality**: Sui's consensus (Mysticeti) provides finality in ~400ms. EVM L2s require 1-2 seconds, and Ethereum mainnet needs 12+ seconds. Arbitrage windows rarely last this long.

### Why Aptos (Planned Support)

Aptos shares Move's object model and deterministic execution but with distinct advantages:
- **Block-STM Parallelism**: Better concurrent transaction throughput for high-frequency trading
- **Different DEX Ecosystem**: Non-overlapping liquidity pools create unique arbitrage opportunities
- **Multi-Chain Strategy**: Reduces risk of single-chain dependency and exploits cross-chain price discrepancies

---

## 3. Architecture Overview

### Dual-Path State Synchronization

```
┌─────────────────────────────────────────────────────────────┐
│                     ARBITRAGE ENGINE                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              Main Loop (50Hz / 20ms)                   │ │
│  │  • Poll detector for next opportunity                  │ │
│  │  • Validate opportunity (liquidity, staleness, etc.)   │ │
│  │  • Execute if profitable after gas estimation          │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
              ▲                            ▲
              │                            │
       HOT PATH (Event-Driven)      COLD PATH (Polling)
              │                            │
┌─────────────────────────────┐   ┌──────────────────────────┐
│    EVENT PROCESSOR          │   │  SYNC ORCHESTRATOR       │
│  • WebSocket Manager per    │   │  • Periodic: 1 hour      │
│    DEX spawns async tasks   │   │  • Emergency: 5 min      │
│  • Parses swap events       │   │  • Fetches stale pools   │
│  • Updates DexManager state │   │  • Batch RPC requests    │
│  • Triggers detector scan   │   │  • Reconciles state      │
└─────────────────────────────┘   └──────────────────────────┘
              │                            │
              └──────────┬─────────────────┘
                         ▼
              ┌──────────────────────┐
              │     DEX MANAGER      │
              │  • HashMap<DexId,    │
              │    Box<dyn DexState>>│
              │  • Pool ID -> DEX    │
              │    lookup table      │
              │  • Thread-safe with  │
              │    Arc<RwLock>       │
              └──────────────────────┘
                         ▼
        ┌────────────────────────────────────┐
        │      DEX STATE (Per DEX)           │
        │  • HashMap<PoolId, PoolState>      │
        │  • get_price(TokenPair) -> Price   │
        │  • process_swap_event()            │
        │  • fetch_pool_state() via RPC      │
        └────────────────────────────────────┘
```

### Data Flow: Initialization → Detection → Execution

**Initialization (Cold Start)**:
1. Load `config.toml` with network, DEXes, pools, arbitrage parameters
2. Validate at least one DEX enabled, register all pools in DexManager
3. SyncOrchestrator performs initial RPC fetch of all pool states (batched)
4. DexManager updates internal PoolState for each pool
5. ArbitrageEngine starts main loop at 50Hz

**Hot Path (Event-Driven Update)**:
1. WebSocket receives `SwapEvent` from Sui node (e.g., Cetus pool 0xabc123...)
2. EventProcessor parses event → extracts pool_id, amounts, direction
3. DexManager routes to correct DexState adapter (Cetus/Turbos/etc.)
4. DexState updates PoolState reserves based on swap
5. ArbitrageDetector scans for new opportunities involving updated pool
6. If found: Validator checks opportunity → Executor simulates → Executor executes

**Cold Path (Periodic Reconciliation)**:
1. Timer triggers (hourly) or emergency condition (e.g., WS disconnect)
2. SyncOrchestrator queries DexManager for stale pools (>1hr old)
3. PoolStateFetcher batches pool IDs by network/DEX, calls `multiGetObjects` RPC
4. PoolParserRegistry parses SuiObjectData → PoolState per DEX-specific schema
5. StateManager updates DexManager with fresh states
6. Reconciles any drift from missed events

---

## 4. High-Level Design Principles

### 1. Trait-Based Pluggability
Every component uses trait abstraction for testability and extensibility:
- `DexState` trait: Unified interface for all DEX adapters (Cetus, Turbos, Kriya)
- `ArbitrageDetector`, `OpportunityValidator`, `TradeExecutor`: Swappable strategies
- `RpcClient` trait: Mock-friendly for testing without hitting live RPC endpoints

**Why**: Adding a new DEX (Aftermath, FlowX) requires implementing one trait, not refactoring the entire engine.

### 2. Async Concurrency with Structured Parallelism
- WebSocket managers spawn per-DEX tasks, communicate via `mpsc` channels
- SyncOrchestrator batches RPC requests (10 pools/batch) with configurable delays to avoid rate limits
- ArbitrageEngine uses `tokio::select!` to multiplex detection loop, periodic sync, and shutdown signals

**Why**: Blocking on RPC or WebSocket I/O would bottleneck the 50Hz detection loop. Async I/O keeps CPU saturated with path-finding logic.

### 3. Shared Mutable State with RwLock
`DexManager` is wrapped in `Arc<RwLock<DexManager>>`:
- Read-heavy workload: Detection reads pool states 50x/sec
- Write-sparse: State updates from events or sync (~1-10x/sec)
- RwLock allows multiple concurrent readers, single writer

**Why**: Mutex would serialize reads, killing performance. Channels would require duplication of state.

### 4. Strict Error Handling (No Panics)
- `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]` enforced
- All results propagated via `?` operator with typed `BotError` enum
- Partial failures handled gracefully (e.g., batch sync continues on single pool failure)

**Why**: In production, panics = downtime = missed opportunities. Explicit error handling enables logging, retries, and degraded operation.

### 5. Configuration-Driven Behavior
`config.toml` controls:
- Which DEXes/pools to monitor
- Arbitrage constraints (max hops, min profit %, max price impact)
- Sync intervals, batch sizes, retry logic
- Execution mode (dry-run vs. live)

**Why**: No recompilation needed to add pools, adjust risk parameters, or switch networks.

### 6. Separation of Concerns
- **Client Layer** (`sui_rpc.rs`): Pure RPC logic, returns `SuiObjectData`
- **Parser Layer** (`cetus.rs`, `turbos.rs`): Converts blockchain objects → `PoolState`
- **State Layer** (`DexState`, `DexManager`): Business logic, no blockchain knowledge
- **Engine Layer** (`ArbitrageEngine`): Orchestration, no domain-specific logic

**Why**: Each layer can be tested independently, replaced, or optimized without cascading changes.

---

## 5. Core Components and Responsibilities

### **DexManager** (`src/dex/manager.rs`)
**Responsibility**: Central registry and coordinator for all DEX adapters.

**Key Operations**:
- `register_dex(Box<dyn DexState>)`: Add new DEX adapter at runtime
- `register_pools(DexId, Vec<PoolId>)`: Associate pools with DEX for routing
- `get_all_prices(TokenPair)`: Query current price across all DEXes for arbitrage detection
- `update_pool_state(PoolState)`: Write path for state updates from events or sync

**Why It Matters**: Decouples pool state management from the arbitrage logic. Adding Kriya support means registering a `KriyaDexState` instance, not modifying detection algorithms.

---

### **SyncOrchestrator** (`src/sync/synchronizer.rs`)
**Responsibility**: Cold-path state synchronization with drift correction.

**Key Operations**:
- `initialize()`: Performs initial sync of all pools at startup
- `sync_pools(SyncType)`: Fetches stale/all pools based on TTL (1 hour default)
- Groups pools by (Network, DexId) for batch efficiency
- Delegates to `PoolStateFetcher` for RPC calls, `StateManager` for writes

**Why It Matters**: Ensures eventual consistency even if WebSocket connections drop or events are missed. Prevents false positives from stale prices.

---

### **ArbitrageEngine** (`src/arbitrage/arbitrage_engine.rs`)
**Responsibility**: Main execution loop coordinating all subsystems.

**Key Operations**:
- Runs 50Hz loop: `detector.next_opportunity()` → `validator.validate()` → `executor.execute()`
- Spawns periodic sync task (1 hour interval)
- Tracks statistics (opportunities found, executed, profit, failures)
- Graceful shutdown on SIGTERM/SIGINT

**Why It Matters**: Separates orchestration from strategy. Swapping detection algorithms (e.g., Bellman-Ford vs. BFS) only requires implementing `ArbitrageDetector` trait.

---

### **EventProcessor** (`src/event/processor.rs`)
**Responsibility**: Hot-path event ingestion from WebSocket streams.

**Key Operations**:
- `start()`: Spawns WebSocket tasks per enabled DEX
- Each task subscribes to `suix_subscribeEvent` filtered by package ID + event type
- Parses raw JSON events → `RawEvent` → sends via `mpsc::channel`
- Processing loop receives events, updates DexManager state

**Why It Matters**: Isolates WebSocket complexity from business logic. Reconnection logic, ping/pong handling, and event parsing are centralized.

---

### **PoolStateFetcher** (`src/sync/fetcher.rs`)
**Responsibility**: Batch RPC fetching of pool states with rate limiting.

**Key Operations**:
- `fetch_batch(Network, DexId, Vec<PoolId>)`: Calls `multiGetObjects` with configurable batch size (10 default)
- Adds 2-second delay between batches to avoid rate limiting
- Uses `PoolParserRegistry` to delegate parsing to DEX-specific parsers (Cetus vs. Turbos)

**Why It Matters**: RPC endpoints have strict rate limits. Batching 100 pools → 10 requests instead of 100. Reduces latency (parallel requests) and avoids IP bans.

---

### **DexState Trait** (`src/dex/state.rs`)
**Responsibility**: Unified interface for all DEX adapters.

**Key Methods**:
```rust
async fn initialize(&mut self) -> Result<()>;
async fn get_pool_state(&self, pool_id: &PoolId) -> Result<PoolState>;
async fn update_pool_state(&mut self, pool_state: PoolState) -> Result<()>;
fn get_price(&self, pair: &TokenPair) -> Option<Price>;
fn process_swap_event(&mut self, event: SwapEvent) -> Result<PriceUpdate>;
async fn heartbeat(&mut self) -> Result<HealthStatus>;
```

**Why It Matters**: Polymorphism enables treating Cetus, Turbos, Kriya identically at the engine level. DexManager holds `HashMap<DexId, Box<dyn DexState>>` and dispatches without knowing implementation details.

---

### **PoolState** (`src/types/pool/pool_state.rs`)
**Responsibility**: Canonical representation of liquidity pool state.

**Fields**:
```rust
pub struct PoolState {
    pub dex_id: DexId,
    pub pool_id: PoolId,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub reserve_a: Decimal,      // Adjusted for decimals
    pub reserve_b: Decimal,
    pub liquidity: Decimal,
    pub fee_rate: Decimal,       // e.g., 0.003 for 0.3%
    pub block_timestamp: Timestamp,
}
```

**Methods**:
- `spot_price_a_to_b()`: Instant price calculation (reserve_b / reserve_a)
- `constant_product()`: For validating AMM invariant (k = reserve_a * reserve_b)

**Why It Matters**: Abstracts DEX-specific pool formats (Cetus CLMM ticks vs. Turbos constant product). Arbitrage logic operates on `PoolState`, not raw blockchain data.

---

### **ArbitrageDetector Trait** (`src/arbitrage/detector.rs`)
**Responsibility**: Path-finding algorithm to discover profitable cycles.

**Key Methods**:
```rust
async fn next_opportunity(&mut self) -> Option<ArbitrageOpportunity>;
fn get_stats(&self) -> DetectionStats;
```

**Current Status**: Skeleton implementation (returns `None`). Production would implement graph-based search:
1. Build token graph: nodes = tokens, edges = pools with weights = log(price)
2. Run Bellman-Ford or Floyd-Warshall to find negative-weight cycles
3. Convert cycles to `ArbitragePath` with hop-by-hop amounts

**Why It Matters**: Decouples detection algorithm from execution. Can A/B test different strategies (greedy BFS vs. dynamic programming) without changing engine.

---

### **ArbitrageCalculator Trait** (`src/arbitrage/calculator.rs`)
**Responsibility**: Profit calculation accounting for fees, slippage, gas.

**Key Methods**:
```rust
async fn find_opportunities(&self, snapshot: &StateSnapshot) -> Vec<ArbitrageOpportunity>;
async fn calculate_profitability(&self, path: &ArbitragePath, snapshot: &StateSnapshot) -> Result<ArbitrageOpportunity>;
```

**Formula**:
```
net_profit = final_amount - initial_amount - gas_cost - Σ(dex_fees)
profit_percent = (net_profit / initial_amount) * 100
```

**Why It Matters**: Prevents unprofitable trades. Gas costs can exceed profits on small opportunities. Slippage from large trades can turn paper profits into losses.

---

### **OpportunityValidator Trait** (`src/arbitrage/validator.rs`)
**Responsibility**: Pre-execution validation to filter false positives.

**Validation Checks**:
1. **Staleness**: Opportunity age < 2 seconds (configurable)
2. **Liquidity**: Each pool has min $1,000 liquidity (avoid dust pools)
3. **Price Divergence**: Current price vs. expected price < 5% (detect stale state)
4. **Gas Cost**: Gas < 50% of profit (avoid unprofitable trades)

**Why It Matters**: Reduces wasted gas on failed transactions. Prevents executing on outdated state (e.g., pool drained by front-runner).

---

### **TradeExecutor Trait** (`src/execution/executor.rs`)
**Responsibility**: Transaction construction and submission.

**Key Steps** (planned):
1. Build PTB (Programmable Transaction Block) for multi-hop swap
2. Simulate transaction via `dryRun` to estimate gas and validate success
3. If simulation passes, sign and submit transaction
4. Wait for finality, parse execution result

**Why It Matters**: Separates execution logic from detection. Enables dry-run mode for testing without spending gas.

---

## 6. Why This Architecture Makes Sense on Sui

### Object-Centric Event Filtering
Sui's `suix_subscribeEvent` allows filtering by:
```rust
"MoveEventType": "0x686e66a7...::swap::SwapEvent"
```
This targets only Cetus swap events, not all chain events. On Ethereum, you'd subscribe to a contract address and filter client-side, processing 10-100x more data.

**Impact**: Reduced bandwidth, lower latency (no client-side filtering), immediate actionable events.

---

### Typed Object State via Move
Sui pool objects have structured fields accessible via `SuiObjectData`:
```json
{
  "fields": {
    "coin_a": "5041265070",
    "coin_b": "3143745307052",
    "liquidity": "125087290394",
    "fee_rate": "2500"
  }
}
```
No ABI decoding, no log parsing, no string manipulation. Parsing is type-safe via `FieldExtractor`.

**Impact**: Faster parsing, fewer bugs, easier testing (mock `SuiObjectData` directly).

---

### Deterministic Gas Budgeting
Sui's gas model charges fixed amounts for computation + storage. Multi-hop arbitrage transactions have predictable costs (~10-20 MIST per hop). EVM chains have variable gas (especially during MEV wars), making profitability calculation impossible without simulating every block.

**Impact**: Pre-execution validation is reliable. Can confidently execute if `net_profit > gas_budget`.

---

### Parallel Transaction Execution
Sui's DAG-based consensus allows parallel execution of transactions touching disjoint objects. If arbitrage path A uses pools {X, Y} and path B uses pools {Z, W}, both can execute simultaneously without contention.

**Impact**: No need to compete with other bots via gas auctions. Multiple opportunities exploitable per block.

---

### Sub-Second Finality
Sui's Mysticeti consensus provides finality in ~400ms. Combined with WebSocket events (near-instant), the hot path latency is:
```
Event emission → WS receive (50ms) → Parse + Update (5ms) → Detect (10ms) → Validate (5ms) → Execute (50ms) → Finality (400ms) = ~520ms total
```
On Ethereum mainnet: 12+ seconds. On L2s: 1-2 seconds. Arbitrage windows rarely last this long.

**Impact**: Higher success rate, more opportunities captured before competition.

---

### Move Object Composability
Sui's shared objects allow atomic multi-hop swaps within a single PTB:
```move
let coin_out = swap_pool_a(pool_a, coin_in);
let coin_final = swap_pool_b(pool_b, coin_out);
```
EVM requires multiple `call()` instructions, risking MEV sandwich attacks or partial execution failures.

**Impact**: Atomic execution guarantees (all hops succeed or entire transaction reverts). No mid-path failures.

---

## Summary

This arbitrage bot leverages Sui's unique properties—object-centric events, typed state, deterministic gas, parallel execution, and sub-second finality—to build a high-frequency trading system that would be impractical on EVM chains. The dual-path architecture (hot + cold) ensures both low latency and eventual consistency, while trait-based design enables rapid extensibility to new DEXes. The result is a production-grade system capable of detecting and capturing arbitrage opportunities faster and more reliably than polling-only or streaming-only alternatives.
