//! # crawlkit-fetcher-reqwest
//!
//! 基于 reqwest 的 HTTP 客户端实现。
//! 作为独立 crate 提供，由 `crawlkit` facade 通过 `fetcher-reqwest` feature 按需引入。

pub mod client;

pub use client::ReqwestClient;
