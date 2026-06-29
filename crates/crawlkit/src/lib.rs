//! # crawlkit
//!
//! 受 Go [colly](https://github.com/gocolly/colly) 启发的 Rust 爬虫工具包。
//!
//! 本 crate 为集成入口（facade），重新导出所有子 crate 的公共 API。
//!
//! ## 设计理念
//! - **可插拔的 HTTP 客户端**：内置 reqwest / wreq 后端，支持代理配置和重试；可通过 `HttpClient` trait 接入自定义客户端
//! - **回调驱动**：类似 colly 的 OnHTML / OnRequest / OnResponse 模式
//! - **异步就绪**：基于 tokio + async-trait，支持并发爬取
//! - **智能内容提取**：支持 Readability 模式和 CSS 选择器提取
//!
//! ## 快速上手
//! ```rust,no_run
//! use crawlkit::Collector;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut c = Collector::new();
//!     c.on_request(|req| {
//!         println!("即将请求: {}", req.url);
//!     });
//!     c.visit("https://example.com").await.unwrap();
//! }
//! ```

pub use crawlkit_core::*;
pub use crawlkit_fetcher::CompositeFetcher;

/// Reqwest 后端（默认启用，可通过 `fetcher-reqwest` feature 控制）
#[cfg(feature = "fetcher-reqwest")]
pub use crawlkit_fetcher_reqwest::ReqwestClient;

/// Wreq 后端（需启用 `fetcher-wreq` feature）
#[cfg(feature = "fetcher-wreq")]
pub use crawlkit_fetcher_wreq::WreqClient;

/// HTML 解析与内容提取工具模块（重新导出）
pub mod html {
    pub use crawlkit_parser::*;
}

pub mod collector;
pub mod log;

pub use collector::{Collector, Element, LimitRule};
