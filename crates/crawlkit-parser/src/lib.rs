//! # crawlkit-parser
//!
//! HTML 解析与内容提取模块。
//! 基于 `scraper`（html5ever）和 `dom_smoothie`（Readability 模式）实现。

pub mod html;

pub use html::{
    extract_absolute_links, extract_article, extract_attributes, extract_content,
    extract_content_by_selector, extract_links, extract_links_by_selector, extract_links_by_xpath,
    extract_readable_content, extract_texts, resolve_url, try_extract_links, Article,
    LinkSelectorType,
};

/// 重新导出 scraper 供上层 crate 构建 Element
pub use scraper;

/// 重新导出 skyscraper 供上层 crate 实现 XPath 元素回调
pub use skyscraper;
