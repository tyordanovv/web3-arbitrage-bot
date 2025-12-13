#![allow(unused_variables, unused_imports, dead_code, unused_mut)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(unused_must_use)]
pub mod arbitrage;
pub mod dex;
pub mod event;
pub mod execution;
pub mod sync;
pub mod types;
pub mod utils;
pub mod client;