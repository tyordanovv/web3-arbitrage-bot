interface implementations: dynamic dispatch has a small overhead due to vtable lookups, while static dispatch with enums allows the compiler to inline and optimize better. For a high-frequency bot (50Hz), this is a good optimization.

Perfect! I've successfully refactored the code to use static dispatch with enums instead of dynamic dispatch with trait objects. The build completes successfully.

  Summary of Changes

  1. Created RpcClientEnum (src/client/client.rs:31-74)

  #[derive(Clone)]
  pub enum RpcClientEnum {
      Sui(SuiRpcClient),
      // Future: Ethereum(EthereumRpcClient),
      // Future: Solana(SolanaRpcClient),
  }

  This enum implements all the same methods as the trait, but uses match statements for static dispatch instead of vtable lookups.

  2. Updated PoolStateFetcher (src/sync/fetcher.rs)

  - Changed field from HashMap<Network, Arc<dyn RpcClient>> to HashMap<Network, Arc<RpcClientEnum>>
  - Wrapped SuiRpcClient in RpcClientEnum::Sui variant
  - Updated all method signatures to use the concrete enum type

  3. Made SuiRpcClient cloneable (src/client/sui_rpc.rs:9)

  - Added #[derive(Clone)] since SuiClient supports cloning

  Performance Benefits: Static vs Dynamic Dispatch

  You're correct about the performance implications:

  Dynamic Dispatch (Arc<dyn RpcClient>)

  - Uses vtable (virtual function table) for method resolution
  - Adds one extra pointer indirection per call
  - Prevents inlining of methods
  - Data stored on heap behind pointer
  - Runtime cost: ~1-5ns per call (negligible for most cases)

  Static Dispatch (RpcClientEnum with match)

  - No vtable - the compiler knows the exact type at compile time
  - Methods can be inlined by the optimizer
  - Code is monomorphized - generates specialized versions
  - Better for hot paths (your 50Hz arbitrage loop)
  - Zero-cost abstraction - as fast as calling methods directly

  When to Use Each Approach

  Use enums (static dispatch) when:
  - Small, known set of types (2-5 variants)
  - Performance-critical hot paths
  - You control all implementations
  - Your case: Arbitrage bot with 50Hz loop ✅

  Use trait objects (dynamic dispatch) when:
  - Unknown/unbounded number of types
  - Plugin systems
  - The 1-5ns overhead doesn't matter

  Adding Future Clients

  To add a new blockchain client (e.g., Ethereum):

  1. Create the client: src/client/ethereum_rpc.rs
  2. Add enum variant:
  pub enum RpcClientEnum {
      Sui(SuiRpcClient),
      Ethereum(EthereumRpcClient),  // New
  }
  3. Update match arms in each method:
  match self {
      RpcClientEnum::Sui(client) => client.batch_get_objects(...).await,
      RpcClientEnum::Ethereum(client) => client.batch_get_objects(...).await,
  }

  The compiler will enforce that you handle all variants, preventing bugs!

  Your type error is now resolved and you have better performance characteristics for your high-frequency trading bot. 🚀
