# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based arbitrage bot for the Sui blockchain that detects and executes cross-DEX arbitrage opportunities. The bot monitors multiple decentralized exchanges (DEXes) in real-time via WebSocket connections, maintains local state, and identifies profitable trading paths.

The architecture implements a dual-path synchronization strategy:
- **Hot path**: Real-time WebSocket event streaming for sub-second latency
- **Cold path**: Periodic RPC-based state synchronization as fallback/reconciliation

## Commands

### Build & Run
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run the bot (uses config.toml)
cargo run

# Run with specific log level
RUST_LOG=debug cargo run

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Run tests with output
cargo test -- --nocapture
```

### Linting
```bash
# Run clippy
cargo clippy

# Run clippy with all features
cargo clippy --all-features

# Format code
cargo fmt
```

## Code Standards

The codebase enforces strict safety rules via lib.rs:
- `#![deny(clippy::unwrap_used)]` - Never use `.unwrap()`
- `#![deny(clippy::expect_used)]` - Never use `.expect()`
- `#![deny(unused_must_use)]` - Always handle Result types

Use `?` for error propagation and proper Result handling throughout.

## Architecture Overview

### Core Components

1. **DexManager** (`src/dex/manager.rs`)
   - Central registry for all DEX adapters
   - Maps pool IDs to their respective DEXes
   - Manages pool registration and health checks
   - Uses builder pattern for initialization with config-driven DEX/pool registration
   - Key methods: `register_dex()`, `register_pools()`, `get_all_prices()`, `update_pool_states()`

2. **SyncOrchestrator** (`src/sync/synchronizer.rs`)
   - Coordinates cold-path state synchronization
   - Works with StateManager and PoolStateFetcher
   - Supports Initial, All, and Stale sync types
   - Batches pool fetches by network and DEX for efficiency
   - Initialized via builder pattern with DexManager, RPC endpoint, and SyncConfig

3. **ArbitrageEngine** (`src/arbitrage/arbitrage_engine.rs`)
   - Main execution loop coordinating all components
   - Runs at 50Hz (20ms intervals) for opportunity detection
   - Periodic sync every hour
   - Uses trait-based composition for components:
     - EventProcessor: Real-time event handling (hot path)
     - ArbitrageDetector: Identifies profitable paths
     - OpportunityValidator: Pre-execution validation
     - TradeExecutor: Executes arbitrage transactions
   - Initialized via builder pattern

4. **EventProcessor** (`src/event/processor.rs`)
   - Manages WebSocket connections per DEX
   - Spawns WebSocketManager tasks for each enabled DEX
   - Routes raw events to appropriate handlers via mpsc channels
   - Currently skeletal - event processing logic incomplete

5. **PoolState** (`src/types/pool/pool_state.rs`)
   - Core state representation: reserves, liquidity, fees, timestamps
   - DEX-specific parsers: CetusPoolParser, TurbosPoolParser
   - Provides spot price calculations and constant product formulas

### Configuration System

The bot uses a hierarchical configuration in `config.toml`:

```toml
[network]
network = "SuiMainnet"
rpc_url = "..."
ws_url = "..."

[[network.dexes]]
id = "Cetus"
package_id = "0x..."
event_type = "SwapEvent"
enabled = true

[[network.dexes.pools]]
address = "0x..."
token_a = { symbol = "USDC", decimals = 6 }
token_b = { symbol = "SUI", decimals = 9 }

[arbitrage]
max_hops = 4
min_profit_percent = 0.5

[execution]
dry_run = true
gas_budget = 10000000

[sync]
max_pools_per_dex = 1000
sync_interval_seconds = 3600
state_ttl_seconds = 3600
batch_size = 10
```

Configuration loading (`src/utils/config.rs`):
- Loads from `config.toml` if present
- Falls back to defaults with environment variable overrides
- Environment vars: `RPC_URL`, `WS_URL`, `PRIVATE_KEY`
- Validation ensures at least one enabled DEX

### Data Flow

1. **Initialization** (main.rs):
   - Load & validate config
   - Build DexManager with config-defined DEXes/pools
   - Build SyncOrchestrator
   - Perform initial state sync via `orchestrator.initialize()`
   - Build ArbitrageEngine with all components
   - Start engine main loop

2. **Hot Path (Event-Driven)**:
   - WebSocket receives DEX event
   - EventProcessor parses and routes to DexManager
   - DexManager updates PoolState
   - ArbitrageDetector checks for new opportunities
   - Validator confirms opportunity viability
   - Executor submits transaction (if not dry-run)

3. **Cold Path (Polling)**:
   - Periodic timer triggers sync
   - SyncOrchestrator fetches stale/all pools
   - PoolStateFetcher batches RPC requests
   - StateManager updates DexManager
   - State reconciliation ensures correctness

### Type System

Key types in `src/types/`:
- `DexId`: Enum for DEX identifiers (Cetus, Turbos, etc.)
- `Network`: Enum for blockchain networks (SuiMainnet, SuiTestnet)
- `ChainAddress`: Enum wrapping blockchain-specific addresses
- `SuiAddress`: Newtype wrapper around ObjectID with FromStr parsing
- `PoolId`: Type alias for ChainAddress
- `PoolState`: Core pool state with reserves, liquidity, fees
- `TokenInfo`: Symbol, decimals, optional address
- `ArbitrageOpportunity`: Path, expected profit, DEX sequence
- `BotError`: Comprehensive error enum with thiserror

### DEX Adapters

Each DEX implements the `DexState` trait (`src/dex/state.rs`):
- `initialize()`: Setup/validation
- `get_price()`: Current price for token pair
- `update_pool_state()`: Apply new state
- `get_all_pool_states()`: Export all tracked pools
- `heartbeat()`: Health check

Example: `CetusDexState` (`src/dex/cetus.rs`)
- Manages HashMap of pool states
- Parses Cetus-specific pool data structures
- Converts CLMM tick data to reserve equivalents

### Adding a New DEX

1. Create adapter in `src/dex/your_dex.rs` implementing `DexState`
2. Add pool parser in `src/types/pool/your_dex.rs` implementing `PoolParser`
3. Update `DexId` enum in `src/types/dex.rs`
4. Update `DexManagerBuilder::create_dex_state()` in `src/dex/manager.rs`
5. Add DEX config to `config.toml`

## Development Notes

### Current State (Branch: feature/init-state-builder)

The bot is in active development. Core infrastructure is complete:
- ✅ Config loading and validation
- ✅ DexManager with pool registration
- ✅ SyncOrchestrator cold path
- ✅ Initial state sync on startup
- ✅ ArbitrageEngine skeleton

Incomplete areas:
- ⚠️ EventProcessor event routing (see TODOs in processor.rs)
- ⚠️ ArbitrageDetector path-finding algorithm
- ⚠️ OpportunityValidator implementation
- ⚠️ TradeExecutor transaction building
- ⚠️ WebSocket reconnection logic
- ⚠️ Heartbeat failure handling (DexManager:136-141)

### Testing

When writing tests:
- Mock external dependencies (Sui RPC, WebSocket)
- Use `mockito` for HTTP mocking (already in dev-dependencies)
- Test both hot and cold path state updates
- Verify state consistency under race conditions
- Test configuration validation edge cases

### Sui SDK Usage

The bot uses `sui-sdk` from GitHub:
- Import types from `sui_sdk::types::base_types` (ObjectID, etc.)
- RPC client in `src/client/sui_rpc.rs` wraps SuiClient
- Parse addresses using `ObjectID::from_hex_literal()`
- Handle sui-sdk errors by converting to `BotError::SuiReadRpc`

### Common Patterns

**Builder Pattern**: DexManager, ArbitrageEngine, SyncOrchestrator all use builders for testability and flexibility.

**Trait-Based Composition**: Engine components use trait objects (`Box<dyn Trait>`) for pluggability.

**Arc<RwLock<T>>**: Shared mutable state (DexManager) uses async-aware RwLock.

**Error Handling**: Always return `Result<T>` using custom `BotError` type. Use `?` operator, never unwrap/expect.

**Logging**: Use `tracing` crate (info!, debug!, warn!, error!) with structured fields.

## Roadmap Reference

See `roadmap.md` for the phased implementation plan (Phases 0-8). Currently completing Phase 6 (Arbitrage Detection) and preparing for Phase 7 (Trade Execution).
