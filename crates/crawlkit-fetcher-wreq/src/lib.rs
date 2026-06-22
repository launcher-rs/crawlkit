//! # crawlkit-fetcher-wreq
//!
//! 基于 wreq 的 HTTP 客户端实现。
//! wreq 是 reqwest 的硬分叉，提供 TLS 指纹模拟（JA3/JA4）能力。
//! 作为独立 crate 提供，由 `crawlkit` facade 通过 `fetcher-wreq` feature 按需引入。

pub mod client;

pub use client::WreqClient;
