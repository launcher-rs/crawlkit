//! # crawlkit-parser
//!
//! HTML 解析与内容提取模块。
//! 基于 `scraper`（html5ever）和 `dom_smoothie`（Readability 模式）实现。
//!
//! ## 模块
//!
//! - `html`：原有基础工具（链接提取、文章提取、可读性）
//! - `types`：共享类型定义
//! - `selector`：预编译 CSS 选择器
//! - `metadata`：页面元数据（OG/Twitter/JSON-LD/Robots）
//! - `text`：正文提取、文本规范化、可读性评分、语言检测
//! - `links`：链接提取、URL 规范化、站内外分类
//! - `content`：结构化内容（标题/列表/表格/代码/引用）
//! - `parser`：HtmlParser 统一入口
//! - `forms`：表单检测与分类
//! - `pagination`：翻页检测
//! - `contact`：联系方式提取
//! - `feeds`：Feed/Sitemap 检测
//! - `fingerprint`：内容指纹与 AMP 检测

pub mod html;
pub mod types;
pub mod selector;
pub mod metadata;
pub mod text;
pub mod links;
pub mod content;
pub mod parser;
pub mod forms;
pub mod pagination;
pub mod contact;
pub mod feeds;
pub mod fingerprint;

// 向后兼容的模块别名
pub mod selectors;

// ============================================================================
// 类型重导出
// ============================================================================

pub use types::{
    ParserError, ParserResult,
    TextContent,
    Heading,
    Link, LinkRel, LinkType,
    Image, ImageLoading,
    ListContent, ListType, ListItem,
    TableContent, TableRow, TableCell,
    CodeBlock,
    Quote,
    PageMetadata, OpenGraph, TwitterCard, RobotsMeta, AlternateLink,
    StructuredData, StructuredDataFormat,
    ParsedContent, ParseStats,
    ParserConfig,
    normalize_whitespace, clean_text, truncate_text,
};

// ============================================================================
// 选择器工具
// ============================================================================

pub use selector::{
    SELECTORS, CachedSelectors,
    get_or_create_selector, parse_selector, try_parse_selector,
    heading_selector,
    CONTENT_SELECTORS, BOILERPLATE_SELECTORS,
    attr_selector, class_selector, id_selector,
    meta_name_selector, meta_property_selector, link_rel_selector,
};

// ============================================================================
// 元数据提取
// ============================================================================

pub use metadata::{
    extract_metadata,
    extract_title, extract_charset, extract_language,
    extract_meta_content, extract_keywords,
    extract_canonical, extract_favicon,
    extract_robots,
    extract_opengraph, extract_twitter_card,
    extract_alternates,
    extract_structured_data, extract_json_ld, extract_microdata,
};

// ============================================================================
// 文本处理
// ============================================================================

pub use text::{
    extract_text as extract_text_content,
    normalize_text, strip_html_tags,
    count_words, count_sentences,
    flesch_reading_ease, flesch_kincaid_grade,
    detect_language,
    is_inline_element,
};

// ============================================================================
// HTML 基础工具（向后兼容）
// ============================================================================

pub use html::{
    Article, LinkSelectorType,
    extract_absolute_links, extract_article, extract_attributes,
    extract_content, extract_content_by_selector,
    extract_links, extract_links_by_selector,
    extract_links_by_xpath, extract_readable_content,
    extract_texts, resolve_url, try_extract_links,
    sanitize_for_xpath,
};

// ============================================================================
// 内容提取（结构化）
// ============================================================================

pub use content::{
    extract_headings, get_main_heading, build_outline,
    extract_paragraphs,
    extract_lists, extract_tables,
    extract_code_blocks, extract_quotes,
};

// ============================================================================
// 链接分析
// ============================================================================

pub use links::{
    LinkStats,
    normalize_url,
    parse_rel_attribute, is_nofollow, is_sponsored, is_ugc,
    filter_internal_links, filter_external_links, filter_followable_links,
    get_external_domains, calculate_link_stats,
};

// ============================================================================
// HtmlParser 入口
// ============================================================================

pub use parser::{
    HtmlParser,
    parse, parse_with_url,
    get_metadata, get_text, get_links,
};

// ============================================================================
// 表单提取
// ============================================================================

pub use forms::{
    Form, FormField, FormType, FieldType, FormMethod, SelectOption,
    extract_forms,
    has_forms, has_login_form, has_search_form,
    get_login_forms, get_search_forms, get_contact_forms,
};

// ============================================================================
// 翻页检测
// ============================================================================

pub use pagination::{
    Pagination, PageUrl, PaginationType,
    extract_pagination, has_pagination,
    get_next_page, get_prev_page,
};

// ============================================================================
// 联系方式提取
// ============================================================================

pub use contact::{
    ContactInfo, Email, EmailSource, Phone, PhoneType,
    Address, Coordinates, SocialLink, SocialPlatform,
    extract_contact_info, extract_emails, extract_phones,
    extract_addresses, extract_social_links,
    has_contact_info, get_emails, get_phones, get_social_links,
};

// ============================================================================
// Feed / Sitemap 检测
// ============================================================================

pub use feeds::{
    FeedInfo, Feed, FeedType, Sitemap, SitemapType, SitemapSource,
    extract_feed_info, has_feeds, get_rss_feed, get_atom_feed, get_feed, get_sitemap,
};

// ============================================================================
// 内容指纹与 AMP
// ============================================================================

pub use fingerprint::{
    ContentFingerprint, AmpInfo, CacheHints,
    generate_fingerprint, fingerprint_document,
    extract_amp_info, extract_cache_hints,
    has_content_changed, content_similarity, is_amp_page, get_amp_url, quick_hash,
};

// ============================================================================
// 第三方重导出
// ============================================================================

/// 重新导出 scraper 供上层 crate 构建 Element
pub use scraper;

/// 重新导出 skyscraper 供上层 crate 实现 XPath 元素回调
pub use skyscraper;
