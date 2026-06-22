//! # crawlkit-parser
//!
//! HTML 解析与内容提取模块。
//! 基于 `scraper`（html5ever）和 `dom_smoothie`（Readability 模式）实现。

pub mod html;

pub use html::{
    extract_article, extract_attributes, extract_content, extract_content_by_selector,
    extract_links, extract_readable_content, extract_texts, resolve_url, Article,
};
