//! # crawlkit
//!
//! 一个受 Go [colly](https://github.com/gocolly/colly) 启发的 Rust 爬虫工具包。
//!
//! ## 设计理念
//! - **可插拔的 HTTP 客户端**：默认使用 reqwest，支持代理配置和重试机制
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

pub mod client;
pub mod collector;
pub mod error;
pub mod fetcher;
pub mod html;
pub mod request;
pub mod response;
pub mod types;

pub use client::{HttpClient, ReqwestClient};
pub use collector::Collector;
pub use error::{CollyError, Result};
pub use fetcher::CompositeFetcher;
pub use request::Request;
pub use response::Response;
pub use types::{ScrapeStats, ScrapedArticle, SiteConfig};
