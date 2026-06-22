//! # crawlkit-fetcher
//!
//! 组合请求器 `CompositeFetcher`，可串联多个 `HttpClient` 实现故障转移。
//!
//! 具体后端子 crate（如 `crawlkit-fetcher-reqwest`）需单独引入，
//! 或通过 `crawlkit` facade 的 feature 统一接入。

pub mod composite;

pub use composite::CompositeFetcher;
