//! # crawlkit-core
//!
//! crawlkit 核心基础库，定义框架所有公共类型、trait 和错误。
//! 此 crate **不含任何 HTTP 或 HTML 解析依赖**，是模块间解耦的基石。

pub mod client;
pub mod error;
pub mod request;
pub mod response;
pub mod types;

pub use client::HttpClient;
pub use error::{CrawlError, Result};
pub use request::Request;
pub use response::Response;
pub use types::{ScrapeStats, ScrapedArticle, SiteConfig};
