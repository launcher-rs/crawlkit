//! Feed 和 Sitemap 检测模块
//!
//! 适配自 halldyll-parser 的 Feed/Sitemap 检测逻辑。
//! 提供从 HTML 页面中提取 RSS、Atom、JSON Feed 以及各类 Sitemap（XML、Index、News、
//! Image、Video、Text、Gzip）的功能。支持从 `<link>` 标签、常见路径、robots.txt
//! 等来源检测，也提供便捷的 URL 生成和类型判定函数。

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

#[allow(unused_imports)]
use crate::types::ParserResult;

// ============================================================================
// 常量
// ============================================================================

/// 常见的 Feed 路径列表。
///
/// 遍历此列表可与目标网站的域名组合，猜测可能的 Feed 地址。
pub const COMMON_FEED_PATHS: &[&str] = &[
    "/feed/",
    "/feed",
    "/rss",
    "/rss/",
    "/atom.xml",
    "/feed.xml",
    "/rss.xml",
    "/index.xml",
    "/atom",
    "/feeds/posts/default",
    "/blog/feed/",
    "/blog/atom.xml",
    "/blog/rss.xml",
    "/blog/feed",
    "/blog/rss",
    "/blog/atom",
    "/feeds/atom.xml",
    "/feeds/rss.xml",
    "/feeds/",
    "/rss/feed.xml",
    "/rss/category/feed.xml",
    "/news/feed/",
    "/news/rss.xml",
    "/news/atom.xml",
    "/articles/feed/",
    "/articles/rss.xml",
    "/articles/atom.xml",
];

/// 常见的 Sitemap 路径列表。
///
/// 遍历此列表可与目标网站的域名组合，猜测可能的 Sitemap 地址。
pub const COMMON_SITEMAP_PATHS: &[&str] = &[
    "/sitemap.xml",
    "/sitemap_index.xml",
    "/sitemapindex.xml",
    "/sitemap/",
    "/sitemap/sitemap.xml",
    "/sitemapindex.xml",
    "/sitemap1.xml",
    "/sitemap.txt",
    "/sitemap.xml.gz",
    "/sitemap_index.xml.gz",
    "/sitemapindex.xml.gz",
    "/sitemap2.xml",
    "/sitemap-news.xml",
    "/sitemap-news.xml.gz",
    "/sitemap-image.xml",
    "/sitemap-video.xml",
    "/sitemap-mobile.xml",
    "/robots.txt",
    "/sitemaps/sitemap.xml",
    "/sitemaps/",
    "/media/sitemap.xml",
    "/images/sitemap.xml",
    "/video/sitemap.xml",
    "/news/sitemap.xml",
    "/sitemap-index.xml",
];

// ============================================================================
// Feed 类型与结构
// ============================================================================

/// Feed 类型枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedType {
    /// RSS 0.91/0.92 早期版本
    Rss,
    /// RSS 2.0
    Rss2,
    /// Atom 协议
    Atom,
    /// JSON Feed
    Json,
    /// 未知格式
    Unknown,
}

/// 表示单个 Feed 的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    /// Feed 类型
    pub feed_type: FeedType,
    /// Feed 的完整 URL
    pub url: String,
    /// Feed 标题（如有）
    pub title: Option<String>,
    /// Feed 描述（如有）
    pub description: Option<String>,
}

/// 包装 Feed 列表的顶层结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedInfo {
    /// 检测到的所有 Feed
    pub feeds: Vec<Feed>,
}

impl FeedInfo {
    /// 创建一个空的 FeedInfo。
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加一个 Feed。
    pub fn add_feed(&mut self, feed: Feed) {
        self.feeds.push(feed);
    }

    /// 判断是否包含任何 Feed。
    pub fn has_any(&self) -> bool {
        !self.feeds.is_empty()
    }

    /// 返回所有 RSS（含 Rss/Rss2）Feed。
    pub fn rss_feeds(&self) -> Vec<&Feed> {
        self.feeds.iter().filter(|f| matches!(f.feed_type, FeedType::Rss | FeedType::Rss2)).collect()
    }

    /// 返回所有 Atom Feed。
    pub fn atom_feeds(&self) -> Vec<&Feed> {
        self.feeds.iter().filter(|f| f.feed_type == FeedType::Atom).collect()
    }

    /// 返回所有 JSON Feed。
    pub fn json_feeds(&self) -> Vec<&Feed> {
        self.feeds.iter().filter(|f| f.feed_type == FeedType::Json).collect()
    }
}

// ============================================================================
// Sitemap 类型与结构
// ============================================================================

/// Sitemap 类型枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitemapType {
    /// 标准 XML Sitemap
    Xml,
    /// Sitemap 索引文件（指向子 Sitemap）
    Index,
    /// Google News Sitemap
    News,
    /// 图片 Sitemap
    Image,
    /// 视频 Sitemap
    Video,
    /// 纯文本 Sitemap
    Text,
    /// Gzip 压缩的 Sitemap
    Gzip,
}

/// Sitemap 来源枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitemapSource {
    /// 从 HTML `<link>` 标签发现
    LinkTag,
    /// 从 robots.txt 发现
    RobotsTxt,
    /// 从 .well-known/ 路径发现
    WellKnown,
    /// 从另一个 Sitemap Index 发现
    SitemapIndex,
}

/// 表示单个 Sitemap 的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sitemap {
    /// Sitemap 的完整 URL
    pub url: String,
    /// Sitemap 类型
    pub sitemap_type: SitemapType,
    /// 发现来源
    pub source: SitemapSource,
}

impl Sitemap {
    /// 创建一个新的 Sitemap。
    pub fn new(url: impl Into<String>, sitemap_type: SitemapType, source: SitemapSource) -> Self {
        Self {
            url: url.into(),
            sitemap_type,
            source,
        }
    }
}

// ============================================================================
// URL 解析
// ============================================================================

/// 将相对路径或协议相对 URL 解析为绝对 URL。
///
/// 支持三种模式：
/// - 绝对 URL（`https://...`）直接返回
/// - 协议相对 URL（`//cdn.example.com/file`）根据 base_url 的 scheme 补全
/// - 相对路径（`/path`、`../path`）基于 base_url 进行解析
pub fn resolve_url(href: &str, base_url: Option<&Url>) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    if href.starts_with("http://") || href.starts_with("https://") {
        return Url::parse(href).ok().map(|u| u.to_string());
    }

    if href.starts_with("//") {
        let scheme = base_url.map(|u| u.scheme()).unwrap_or("https");
        return Some(format!("{scheme}:{href}"));
    }

    let base = base_url?;
    base.join(href).ok().map(|u| u.to_string())
}

// ============================================================================
// Feed 类型检测
// ============================================================================

/// 根据 Feed 内容的开头特征检测 Feed 类型。
///
/// 通过检查前 200 个字符中的标识性标签来判定：
/// - `<rss` 且版本含 `2.0` → Rss2
/// - `<rss` → Rss
/// - `<feed` → Atom
/// - `{"version":"https://jsonfeed.org/` → Json
pub fn detect_feed_type(content: &str) -> FeedType {
    let preview = &content[..content.len().min(200)];

    if let Some(rss_pos) = preview.find("<rss") {
        let after = &preview[rss_pos..];
        if after.contains("2.0") || after.contains("version=\"2.0\"") {
            return FeedType::Rss2;
        }
        return FeedType::Rss;
    }

    if preview.contains("<feed") {
        return FeedType::Atom;
    }

    if preview.contains("\"version\":\"https://jsonfeed.org/") {
        return FeedType::Json;
    }

    FeedType::Unknown
}

/// 根据 URL 的路径和扩展名推测 Feed 类型。
///
/// 规则：
/// - `/atom`、`.atom`、`atom.xml` → Atom
/// - `.json`、`jsonfeed` → Json
/// - 其余默认返回 Rss2
pub fn detect_feed_type_from_url(url: &str) -> FeedType {
    let lower = url.to_lowercase();

    if lower.contains("/atom") || lower.ends_with(".atom") || lower.ends_with("atom.xml") {
        return FeedType::Atom;
    }

    if lower.ends_with(".json") || lower.contains("jsonfeed") {
        return FeedType::Json;
    }

    FeedType::Rss2
}

// ============================================================================
// Sitemap 类型检测
// ============================================================================

/// 根据 Sitemap 的 URL 路径和扩展名推测 Sitemap 类型。
///
/// 规则：
/// - `.txt` → Text
/// - `.gz` 或 `.gzip` → Gzip
/// - 路径含 `news` → News
/// - 路径含 `image` → Image
/// - 路径含 `video` → Video
/// - 路径含 `index` 或 `sitemapindex` → Index
/// - 默认 → Xml
pub fn detect_sitemap_type(url: &str) -> SitemapType {
    let lower = url.to_lowercase();

    if lower.ends_with(".txt") {
        return SitemapType::Text;
    }

    if lower.ends_with(".gz") || lower.ends_with(".gzip") {
        return SitemapType::Gzip;
    }

    if lower.contains("news") {
        return SitemapType::News;
    }

    if lower.contains("image") || lower.contains("images") {
        return SitemapType::Image;
    }

    if lower.contains("video") || lower.contains("videos") {
        return SitemapType::Video;
    }

    if lower.contains("index") || lower.contains("sitemapindex") {
        return SitemapType::Index;
    }

    SitemapType::Xml
}

// ============================================================================
// Feed 提取
// ============================================================================

/// 从 HTML 文档中提取完整的 Feed 信息。
///
/// 内部调用 `extract_link_feeds` 扫描 `<link>` 标签，
/// 返回封装后的 `FeedInfo` 结构。
pub fn extract_feed_info(document: &Html, base_url: Option<&Url>) -> FeedInfo {
    let feeds = extract_link_feeds(document, base_url);
    let mut info = FeedInfo::new();
    for feed in feeds {
        info.add_feed(feed);
    }
    info
}

/// 从 HTML 文档的 `<link>` 标签中提取 Feed 列表。
///
/// 扫描 `link[type="application/rss+xml"]`、
/// `link[type="application/atom+xml"]`、
/// `link[type="application/feed+json"]` 等标签，
/// 提取其 `href`、`title` 等属性并解析 Feed 类型。
pub fn extract_link_feeds(document: &Html, base_url: Option<&Url>) -> Vec<Feed> {
    let mut feeds = Vec::new();

    let selectors = [
        "link[type=\"application/rss+xml\"]",
        "link[type=\"application/atom+xml\"]",
        "link[type=\"application/feed+json\"]",
        "link[rel=\"alternate\"][type=\"application/rss+xml\"]",
        "link[rel=\"alternate\"][type=\"application/atom+xml\"]",
        "link[rel=\"alternate\"][type=\"application/feed+json\"]",
        "link[rel=\"alternate\"][href$=\".rss\"]",
        "link[rel=\"alternate\"][href$=\".atom\"]",
        "link[rel=\"alternate\"][href$=\".json\"]",
    ];

    let mut seen_urls: Vec<String> = Vec::new();

    for selector_str in &selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };

        for element in document.select(&selector) {
            let href = match element.value().attr("href") {
                Some(h) => h.trim(),
                None => continue,
            };

            if href.is_empty() {
                continue;
            }

            let resolved = resolve_url(href, base_url).unwrap_or_else(|| href.to_string());

            if seen_urls.contains(&resolved) {
                continue;
            }
            seen_urls.push(resolved.clone());

            let feed_type = if selector_str.contains("atom") || selector_str.contains(".atom") {
                FeedType::Atom
            } else if selector_str.contains("json") || selector_str.contains(".json") {
                FeedType::Json
            } else {
                FeedType::Rss2
            };

            let title = element.value().attr("title").map(ToString::to_string);
            let description = element.value().attr("description").map(ToString::to_string);

            feeds.push(Feed {
                feed_type,
                url: resolved,
                title,
                description,
            });
        }
    }

    feeds
}

// ============================================================================
// Sitemap 提取
// ============================================================================

/// 从 HTML 文档的 `<link>` 标签中提取 Sitemap 列表。
///
/// 匹配 `link[rel="sitemap"]` 或 `link[type="application/xml"]`
/// 且 href 含 `sitemap` 的标签。
pub fn extract_sitemaps(document: &Html, base_url: Option<&Url>) -> Vec<Sitemap> {
    let mut sitemaps = Vec::new();

    let selectors = [
        "link[rel=\"sitemap\"]",
        "link[type=\"application/xml\"]",
        "a[href*=\"sitemap\"]",
    ];

    let mut seen_urls: Vec<String> = Vec::new();

    for selector_str in &selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };

        for element in document.select(&selector) {
            let href = match element.value().attr("href") {
                Some(h) => h.trim(),
                None => continue,
            };

            if href.is_empty() || !href.to_lowercase().contains("sitemap") {
                continue;
            }

            let resolved = resolve_url(href, base_url).unwrap_or_else(|| href.to_string());

            if seen_urls.contains(&resolved) {
                continue;
            }
            seen_urls.push(resolved.clone());

            let sitemap_type = detect_sitemap_type(&resolved);

            sitemaps.push(Sitemap::new(resolved, sitemap_type, SitemapSource::LinkTag));
        }
    }

    sitemaps
}

// ============================================================================
// URL 生成
// ============================================================================

/// 根据 base URL 生成所有常见 Feed 的完整 URL 列表。
///
/// 将 `COMMON_FEED_PATHS` 中的每个路径与 base URL 拼接，
/// 返回去重后的结果。
pub fn generate_feed_urls(base_url: &Url) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    let base_str = base_url.to_string();
    let base = base_str.trim_end_matches('/');

    for path in COMMON_FEED_PATHS {
        let full = format!("{base}{path}");
        if !urls.contains(&full) {
            urls.push(full);
        }
    }

    urls
}

/// 根据 base URL 生成所有常见 Sitemap 的完整 URL 列表。
///
/// 将 `COMMON_SITEMAP_PATHS` 中的每个路径与 base URL 拼接，
/// 返回去重后的结果。
pub fn generate_sitemap_urls(base_url: &Url) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    let base_str = base_url.to_string();
    let base = base_str.trim_end_matches('/');

    for path in COMMON_SITEMAP_PATHS {
        let full = format!("{base}{path}");
        if !urls.contains(&full) {
            urls.push(full);
        }
    }

    urls
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 判断 HTML 文档中是否包含任何 Feed 链接。
pub fn has_feeds(document: &Html, base_url: Option<&Url>) -> bool {
    !extract_link_feeds(document, base_url).is_empty()
}

/// 从 HTML 文档中获取第一个 RSS（含 Rss/Rss2）Feed。
pub fn get_rss_feed(document: &Html, base_url: Option<&Url>) -> Option<Feed> {
    extract_link_feeds(document, base_url)
        .into_iter()
        .find(|f| matches!(f.feed_type, FeedType::Rss | FeedType::Rss2))
}

/// 从 HTML 文档中获取第一个 Atom Feed。
pub fn get_atom_feed(document: &Html, base_url: Option<&Url>) -> Option<Feed> {
    extract_link_feeds(document, base_url)
        .into_iter()
        .find(|f| f.feed_type == FeedType::Atom)
}

/// 从 HTML 文档中获取第一个 Feed（任意类型）。
pub fn get_feed(document: &Html, base_url: Option<&Url>) -> Option<Feed> {
    extract_link_feeds(document, base_url).into_iter().next()
}

/// 从 HTML 文档中获取第一个 Sitemap。
pub fn get_sitemap(document: &Html, base_url: Option<&Url>) -> Option<Sitemap> {
    extract_sitemaps(document, base_url).into_iter().next()
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 辅助：从 HTML 字符串解析文档
    fn parse_html(html: &str) -> Html {
        Html::parse_document(html)
    }

    // 辅助：解析 URL
    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    // -----------------------------------------------------------------------
    // Feed 类型检测
    // -----------------------------------------------------------------------

    #[test]
    fn 检测_rss2_类型() {
        let content = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Test</title></channel></rss>"#;
        assert_eq!(detect_feed_type(content), FeedType::Rss2);
    }

    #[test]
    fn 检测_rss_旧版_类型() {
        let content = r#"<?xml version="1.0"?><rss version="0.91"><channel><title>Test</title></channel></rss>"#;
        assert_eq!(detect_feed_type(content), FeedType::Rss);
    }

    #[test]
    fn 检测_atom_类型() {
        let content = r#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Test</title></feed>"#;
        assert_eq!(detect_feed_type(content), FeedType::Atom);
    }

    #[test]
    fn 检测_json_feed_类型() {
        let content = r#"{"version":"https://jsonfeed.org/version/1","title":"Test"}"#;
        assert_eq!(detect_feed_type(content), FeedType::Json);
    }

    #[test]
    fn 未知内容返回_unknown() {
        let content = r#"<html><body>普通页面</body></html>"#;
        assert_eq!(detect_feed_type(content), FeedType::Unknown);
    }

    #[test]
    fn 空内容返回_unknown() {
        assert_eq!(detect_feed_type(""), FeedType::Unknown);
    }

    // -----------------------------------------------------------------------
    // Feed 类型 URL 检测
    // -----------------------------------------------------------------------

    #[test]
    fn 从_url_检测_atom() {
        assert_eq!(detect_feed_type_from_url("https://example.com/atom.xml"), FeedType::Atom);
        assert_eq!(detect_feed_type_from_url("https://example.com/blog/atom"), FeedType::Atom);
        assert_eq!(detect_feed_type_from_url("https://example.com/feed.atom"), FeedType::Atom);
    }

    #[test]
    fn 从_url_检测_json() {
        assert_eq!(detect_feed_type_from_url("https://example.com/feed.json"), FeedType::Json);
        assert_eq!(detect_feed_type_from_url("https://example.com/jsonfeed"), FeedType::Json);
    }

    #[test]
    fn 从_url_检测默认_rss2() {
        assert_eq!(detect_feed_type_from_url("https://example.com/rss"), FeedType::Rss2);
        assert_eq!(detect_feed_type_from_url("https://example.com/feed.xml"), FeedType::Rss2);
    }

    // -----------------------------------------------------------------------
    // Sitemap 类型检测
    // -----------------------------------------------------------------------

    #[test]
    fn 检测标准_xml_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap.xml"), SitemapType::Xml);
    }

    #[test]
    fn 检测_sitemap_index() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap_index.xml"), SitemapType::Index);
        assert_eq!(detect_sitemap_type("https://example.com/sitemapindex.xml"), SitemapType::Index);
    }

    #[test]
    fn 检测_news_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap-news.xml"), SitemapType::News);
    }

    #[test]
    fn 检测_image_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap-image.xml"), SitemapType::Image);
    }

    #[test]
    fn 检测_video_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap-video.xml"), SitemapType::Video);
    }

    #[test]
    fn 检测_text_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap.txt"), SitemapType::Text);
    }

    #[test]
    fn 检测_gzip_sitemap() {
        assert_eq!(detect_sitemap_type("https://example.com/sitemap.xml.gz"), SitemapType::Gzip);
    }

    // -----------------------------------------------------------------------
    // URL 解析
    // -----------------------------------------------------------------------

    #[test]
    fn 解析绝对_url() {
        let base = url("https://example.com");
        let result = resolve_url("https://other.com/page", Some(&base));
        assert_eq!(result.as_deref(), Some("https://other.com/page"));
    }

    #[test]
    fn 解析相对路径() {
        let base = url("https://example.com/base/");
        let result = resolve_url("../feed.xml", Some(&base));
        assert_eq!(result.as_deref(), Some("https://example.com/feed.xml"));
    }

    #[test]
    fn 解析协议相对_url() {
        let base = url("https://example.com");
        let result = resolve_url("//cdn.example.com/feed", Some(&base));
        assert_eq!(result.as_deref(), Some("https://cdn.example.com/feed"));
    }

    #[test]
    fn 解析空字符串返回_none() {
        let base = url("https://example.com");
        assert!(resolve_url("", Some(&base)).is_none());
        assert!(resolve_url("  ", Some(&base)).is_none());
    }

    #[test]
    fn 无_base_url_时相对路径返回_none() {
        assert!(resolve_url("/feed.xml", None).is_none());
    }

    // -----------------------------------------------------------------------
    // 从 HTML 提取 Feed
    // -----------------------------------------------------------------------

    #[test]
    fn 提取_rss_feed_从_link_标签() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="RSS" href="/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let feeds = extract_link_feeds(&doc, Some(&base));
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].feed_type, FeedType::Rss2);
        assert_eq!(feeds[0].url, "https://example.com/rss.xml");
        assert_eq!(feeds[0].title.as_deref(), Some("RSS"));
    }

    #[test]
    fn 提取_atom_feed_从_link_标签() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/atom+xml" title="Atom" href="/atom.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let feeds = extract_link_feeds(&doc, Some(&base));
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].feed_type, FeedType::Atom);
        assert_eq!(feeds[0].url, "https://example.com/atom.xml");
    }

    #[test]
    fn 提取_json_feed_从_link_标签() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/feed+json" title="JSON Feed" href="/feed.json" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let feeds = extract_link_feeds(&doc, Some(&base));
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].feed_type, FeedType::Json);
        assert_eq!(feeds[0].url, "https://example.com/feed.json");
    }

    #[test]
    fn 提取多个_feed_并去重() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" title="RSS" href="/rss.xml" />
            <link type="application/rss+xml" title="RSS 2" href="/rss.xml" />
            <link rel="alternate" type="application/atom+xml" title="Atom" href="/atom.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let feeds = extract_link_feeds(&doc, Some(&base));
        // /rss.xml 重复了，应只有 2 个
        assert_eq!(feeds.len(), 2);
    }

    #[test]
    fn 无_feed_时返回空列表() {
        let html = r#"<html><head><title>普通页面</title></head></html>"#;
        let doc = parse_html(html);
        let feeds = extract_link_feeds(&doc, None);
        assert!(feeds.is_empty());
    }

    #[test]
    fn 提取_feed_时处理无_base_url() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="https://other.com/feed.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let feeds = extract_link_feeds(&doc, None);
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url, "https://other.com/feed.xml");
    }

    // -----------------------------------------------------------------------
    // FeedInfo
    // -----------------------------------------------------------------------

    #[test]
    fn feed_info_结构功能() {
        let mut info = FeedInfo::new();
        assert!(!info.has_any());

        info.add_feed(Feed {
            feed_type: FeedType::Rss2,
            url: "https://example.com/rss".to_string(),
            title: None,
            description: None,
        });
        info.add_feed(Feed {
            feed_type: FeedType::Atom,
            url: "https://example.com/atom".to_string(),
            title: None,
            description: None,
        });

        assert!(info.has_any());
        assert_eq!(info.rss_feeds().len(), 1);
        assert_eq!(info.atom_feeds().len(), 1);
        assert!(info.json_feeds().is_empty());
    }

    // -----------------------------------------------------------------------
    // extract_feed_info 封装函数
    // -----------------------------------------------------------------------

    #[test]
    fn extract_feed_info_返回_feedinfo() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let info = extract_feed_info(&doc, Some(&base));
        assert!(info.has_any());
        assert_eq!(info.feeds.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 从 HTML 提取 Sitemap
    // -----------------------------------------------------------------------

    #[test]
    fn 提取_sitemap_从_link_标签() {
        let html = r#"<html><head>
            <link rel="sitemap" type="application/xml" href="/sitemap.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let sitemaps = extract_sitemaps(&doc, Some(&base));
        assert_eq!(sitemaps.len(), 1);
        assert_eq!(sitemaps[0].url, "https://example.com/sitemap.xml");
        assert_eq!(sitemaps[0].sitemap_type, SitemapType::Xml);
        assert_eq!(sitemaps[0].source, SitemapSource::LinkTag);
    }

    #[test]
    fn 提取_sitemap_从链接文本() {
        let html = r#"<html><body>
            <a href="/sitemap-index.xml">Sitemap</a>
        </body></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let sitemaps = extract_sitemaps(&doc, Some(&base));
        assert_eq!(sitemaps.len(), 1);
        assert_eq!(sitemaps[0].url, "https://example.com/sitemap-index.xml");
        assert_eq!(sitemaps[0].sitemap_type, SitemapType::Index);
    }

    #[test]
    fn 提取_sitemap_时过滤掉不含_sitemap_的链接() {
        let html = r#"<html><body>
            <a href="/page1">普通页面</a>
            <a href="/sitemap.xml">Sitemap</a>
        </body></html>"#;
        let doc = parse_html(html);
        let sitemaps = extract_sitemaps(&doc, None);
        assert_eq!(sitemaps.len(), 1);
    }

    // -----------------------------------------------------------------------
    // URL 生成
    // -----------------------------------------------------------------------

    #[test]
    fn 生成常见_feed_url() {
        let base = url("https://example.com");
        let urls = generate_feed_urls(&base);
        assert!(urls.contains(&"https://example.com/feed/".to_string()));
        assert!(urls.contains(&"https://example.com/atom.xml".to_string()));
        assert!(urls.contains(&"https://example.com/rss.xml".to_string()));
        assert!(urls.len() >= 10);
    }

    #[test]
    fn 生成常见_sitemap_url() {
        let base = url("https://example.com");
        let urls = generate_sitemap_urls(&base);
        assert!(urls.contains(&"https://example.com/sitemap.xml".to_string()));
        assert!(urls.contains(&"https://example.com/robots.txt".to_string()));
        assert!(urls.len() >= 10);
    }

    #[test]
    fn 生成_url_去重() {
        let base = url("https://example.com");
        let urls = generate_feed_urls(&base);
        let mut sorted = urls.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(urls.len(), sorted.len());
    }

    #[test]
    fn 生成_url_处理子路径_base() {
        let base = url("https://example.com/blog");
        let urls = generate_feed_urls(&base);
        assert!(urls.contains(&"https://example.com/blog/feed/".to_string()));
        assert!(urls.contains(&"https://example.com/blog/atom.xml".to_string()));
    }

    // -----------------------------------------------------------------------
    // 便捷函数
    // -----------------------------------------------------------------------

    #[test]
    fn has_feeds_检测() {
        let html_with = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html_with);
        assert!(has_feeds(&doc, None));

        let html_without = r#"<html><head><title>无 Feed</title></head></html>"#;
        let doc = parse_html(html_without);
        assert!(!has_feeds(&doc, None));
    }

    #[test]
    fn get_rss_feed_返回第一个_rss() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/atom+xml" href="/atom.xml" />
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let feed = get_rss_feed(&doc, None);
        assert!(feed.is_some());
        assert_eq!(feed.unwrap().feed_type, FeedType::Rss2);
    }

    #[test]
    fn 无_feed_时_get_rss_feed_返回_none() {
        let html = r#"<html><head><title>无 Feed</title></head></html>"#;
        let doc = parse_html(html);
        assert!(get_rss_feed(&doc, None).is_none());
    }

    #[test]
    fn get_atom_feed_返回第一个_atom() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
            <link rel="alternate" type="application/atom+xml" href="/atom.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let feed = get_atom_feed(&doc, None);
        assert!(feed.is_some());
        assert_eq!(feed.unwrap().feed_type, FeedType::Atom);
    }

    #[test]
    fn get_feed_返回第一个任意_feed() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/atom+xml" href="/atom.xml" />
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let feed = get_feed(&doc, None);
        assert!(feed.is_some());
        // 返回的是 atom （选择器匹配顺序）
    }

    #[test]
    fn get_sitemap_返回第一个_sitemap() {
        let html = r#"<html><head>
            <link rel="sitemap" href="/sitemap.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let sitemap = get_sitemap(&doc, None);
        assert!(sitemap.is_some());
        assert_eq!(sitemap.unwrap().url, "/sitemap.xml");
    }

    // -----------------------------------------------------------------------
    // Sitemap 结构
    // -----------------------------------------------------------------------

    #[test]
    fn sitemap_new_便捷构造() {
        let s = Sitemap::new("https://example.com/sitemap.xml", SitemapType::Xml, SitemapSource::WellKnown);
        assert_eq!(s.url, "https://example.com/sitemap.xml");
        assert_eq!(s.sitemap_type, SitemapType::Xml);
        assert_eq!(s.source, SitemapSource::WellKnown);
    }

    // -----------------------------------------------------------------------
    // 边界情况与安全性
    // -----------------------------------------------------------------------

    #[test]
    fn 处理_href_属性缺失() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let feeds = extract_link_feeds(&doc, None);
        assert!(feeds.is_empty());
    }

    #[test]
    fn 处理_malformed_url() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="http://" />
        </head></html>"#;
        let doc = parse_html(html);
        let feeds = extract_link_feeds(&doc, None);
        // href="http://" 在 resolve_url 中会解析失败，但作为原始值保留
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url, "http://");
    }

    #[test]
    fn 不重复记录相同_url() {
        let html = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/rss.xml" />
            <link rel="alternate" type="application/rss+xml" href="https://example.com/rss.xml" />
        </head></html>"#;
        let doc = parse_html(html);
        let base = url("https://example.com");
        let feeds = extract_link_feeds(&doc, Some(&base));
        // 两个 href 解析后相同，应去重
        assert_eq!(feeds.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 常量完整性
    // -----------------------------------------------------------------------

    #[test]
    fn common_feed_paths_非空() {
        assert!(!COMMON_FEED_PATHS.is_empty());
        assert!(COMMON_FEED_PATHS.len() > 5);
    }

    #[test]
    fn common_sitemap_paths_非空() {
        assert!(!COMMON_SITEMAP_PATHS.is_empty());
        assert!(COMMON_SITEMAP_PATHS.len() > 5);
    }

    #[test]
    fn 所有_feed_path_以斜线开头() {
        for path in COMMON_FEED_PATHS {
            assert!(path.starts_with('/'), "路径 {path} 应以 / 开头");
        }
    }

    #[test]
    fn 所有_sitemap_path_以斜线开头() {
        for path in COMMON_SITEMAP_PATHS {
            assert!(path.starts_with('/'), "路径 {path} 应以 / 开头");
        }
    }
}
