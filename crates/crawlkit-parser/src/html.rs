//! HTML 解析工具集
//!
//! 提供链接提取、文章内容提取、可读性提取等实用功能。
//! 底层使用 `scraper`（基于 html5ever）和 `dom_smoothie`（Readability 模式）。

use dom_smoothie::{Config, TextMode};
use scraper::{Html, Selector};
use skyscraper::{
    html as xpath_html,
    xpath::{
        self,
        grammar::{
            data_model::{Node, XpathItem},
            NonTreeXpathNode, XpathItemTreeNodeData,
        },
        XpathItemTree,
    },
};
use url::Url;

use crawlkit_core::error::{CrawlError, Result};

/// 链接选择器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSelectorType {
    /// CSS 选择器，例如 `a[href]`
    Css,
    /// XPath 表达式，例如 `//a/@href`
    Xpath,
}

/// 从 HTML 中提取所有匹配 CSS 选择器的链接（href 属性）
///
/// - `html_content`: 完整 HTML 字符串
/// - `selector`: CSS 选择器，如 `"a[href]"` 或 `"div.news a"`
///
/// 返回去重后的链接列表
pub fn extract_links(html_content: &str, selector: &str) -> Vec<String> {
    try_extract_links(html_content, selector).unwrap_or_default()
}

/// 从 HTML 中提取所有匹配 CSS 选择器的链接（href 属性）
///
/// 和 `extract_links` 相比，此函数会返回选择器解析错误，适合需要严格错误处理的场景。
pub fn try_extract_links(html_content: &str, selector: &str) -> Result<Vec<String>> {
    let document = Html::parse_document(html_content);
    let sel = Selector::parse(selector).map_err(|e| CrawlError::Selector {
        selector: selector.to_string(),
        message: e.to_string(),
    })?;

    let mut seen = std::collections::HashSet::new();
    let mut links = Vec::new();

    for element in document.select(&sel) {
        if let Some(href) = element.value().attr("href") {
            if seen.insert(href.to_string()) {
                links.push(href.to_string());
            }
        }
    }

    Ok(links)
}

/// 按选择器类型提取链接。
///
/// XPath 表达式既可以指向属性节点（如 `//a/@href`），也可以指向元素节点（如 `//a`）。
pub fn extract_links_by_selector(
    html_content: &str,
    selector: &str,
    selector_type: LinkSelectorType,
) -> Result<Vec<String>> {
    match selector_type {
        LinkSelectorType::Css => try_extract_links(html_content, selector),
        LinkSelectorType::Xpath => extract_links_by_xpath(html_content, selector),
    }
}

/// 使用 XPath 表达式提取链接。
pub fn extract_links_by_xpath(html_content: &str, selector: &str) -> Result<Vec<String>> {
    let document = xpath_html::parse(html_content).map_err(|e| CrawlError::Html(e.to_string()))?;
    let tree = XpathItemTree::from(&document);
    let xpath_expr = xpath::parse(selector).map_err(|e| CrawlError::Selector {
        selector: selector.to_string(),
        message: e.to_string(),
    })?;
    let item_set = xpath_expr
        .apply(&tree)
        .map_err(|e| CrawlError::Html(e.to_string()))?;

    let mut seen = std::collections::HashSet::new();
    let mut links = Vec::new();

    for item in &item_set {
        let href = match item {
            XpathItem::Node(Node::NonTreeNode(NonTreeXpathNode::AttributeNode(attribute))) => {
                attribute.value.clone()
            }
            XpathItem::Node(Node::TreeNode(tree_node)) => match tree_node.data {
                XpathItemTreeNodeData::ElementNode(element) => element
                    .get_attribute("href")
                    .map(str::to_string)
                    .unwrap_or_default(),
                _ => String::new(),
            },
            _ => String::new(),
        };

        let href = href.trim();
        if !href.is_empty() && seen.insert(href.to_string()) {
            links.push(href.to_string());
        }
    }

    Ok(links)
}

/// 将相对 URL 转为绝对 URL
///
/// - `base_url`: 基准 URL（当前页面）
/// - `relative`: 相对路径或完整 URL
pub fn resolve_url(base_url: &str, relative: &str) -> Option<String> {
    let relative = relative.trim();
    if should_skip_link(relative) {
        return None;
    }
    let base = Url::parse(base_url).ok()?;
    let resolved = base.join(relative).ok()?;
    match resolved.scheme() {
        "http" | "https" => Some(resolved.to_string()),
        _ => None,
    }
}

/// 提取并解析为绝对 URL，自动去重和过滤非页面链接。
pub fn extract_absolute_links(
    html_content: &str,
    selector: &str,
    selector_type: LinkSelectorType,
    base_url: &str,
) -> Result<Vec<String>> {
    let links = extract_links_by_selector(html_content, selector, selector_type)?;
    let mut seen = std::collections::HashSet::new();
    let mut absolute_links = Vec::new();

    for link in links {
        if let Some(url) = resolve_url(base_url, &link) {
            if seen.insert(url.clone()) {
                absolute_links.push(url);
            }
        }
    }

    Ok(absolute_links)
}

fn should_skip_link(link: &str) -> bool {
    link.is_empty()
        || link.starts_with('#')
        || link.starts_with("javascript:")
        || link.starts_with("mailto:")
        || link.starts_with("tel:")
        || link.starts_with("data:")
}

/// 文章内容提取结果
#[derive(Debug, Clone, Default)]
pub struct Article {
    /// 文章标题
    pub title: String,
    /// 文章正文（纯文本）
    pub content: String,
    /// 发布日期
    pub date: Option<String>,
    /// 作者
    pub author: Option<String>,
    /// 描述/摘要
    pub description: Option<String>,
}

/// 从 HTML 中提取文章内容
///
/// 使用启发式规则提取：按优先级尝试多种常见 DOM 结构。
///
/// # 提取策略
/// 1. 优先查找 `<article>` 标签
/// 2. 查找 `article-body` / `post-content` / `entry-content` 等常见 class
/// 3. 查找 `<h1>` 作为标题，最大的 `<div>` 块作为正文
pub fn extract_article(html_content: &str, _base_url: &str) -> Article {
    let document = Html::parse_document(html_content);
    let mut article = Article::default();

    article.title = extract_title(&document);
    article.content = extract_content_heuristic(&document);
    article.date = extract_meta_content(&document, "date")
        .or_else(|| extract_meta_content(&document, "article:published_time"));
    article.author = extract_meta_content(&document, "author")
        .or_else(|| extract_meta_content(&document, "article:author"));
    article.description = extract_meta_content(&document, "description")
        .or_else(|| extract_meta_content(&document, "og:description"));

    article
}

/// 提取页面标题：优先 og:title → h1 → <title>
fn extract_title(document: &Html) -> String {
    if let Ok(sel) = Selector::parse(r#"meta[property="og:title"]"#) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                if !content.is_empty() {
                    return content.to_string();
                }
            }
        }
    }
    if let Ok(sel) = Selector::parse("h1") {
        if let Some(el) = document.select(&sel).next() {
            let text: String = el.text().collect::<Vec<_>>().join("").trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    if let Ok(sel) = Selector::parse("title") {
        if let Some(el) = document.select(&sel).next() {
            let text: String = el.text().collect::<Vec<_>>().join("").trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

/// 提取正文：按优先级尝试多种策略
fn extract_content_heuristic(document: &Html) -> String {
    // 策略 1：<article> 标签
    if let Ok(sel) = Selector::parse("article") {
        if let Some(el) = document.select(&sel).next() {
            let text = element_to_text(&el);
            if text.len() > 100 {
                return text;
            }
        }
    }

    // 策略 2：常见文章容器 class
    let content_selectors = &[
        "article-body",
        "post-content",
        "entry-content",
        "article-content",
        "news-content",
        "story-body",
        ".content-article",
        "#article-body",
        ".article-body",
        ".post-body",
    ];

    for selector_str in content_selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&sel).next() {
                let text = element_to_text(&el);
                if text.len() > 100 {
                    return text;
                }
            }
        }
    }

    // 策略 3：找最大的文本块（启发式兜底）
    if let Ok(sel) = Selector::parse("div") {
        let divs: Vec<_> = document.select(&sel).collect();
        let mut best = String::new();
        for div in divs {
            let text = element_to_text(&div);
            if text.len() > 200 && text.len() < 50_000 && text.len() > best.len() {
                best = text;
            }
        }
        if !best.is_empty() {
            return best;
        }
    }

    String::new()
}

/// 从 <meta> 标签提取 content 属性
fn extract_meta_content(document: &Html, name: &str) -> Option<String> {
    let sel_str = format!(r#"meta[name="{name}"]"#);
    if let Ok(sel) = Selector::parse(&sel_str) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let content = content.trim().to_string();
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
    }
    let sel_str = format!(r#"meta[property="{name}"]"#);
    if let Ok(sel) = Selector::parse(&sel_str) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(content) = el.value().attr("content") {
                let content = content.trim().to_string();
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
    }
    None
}

// ──────────────────────────────────────────────
// 可读性提取（基于 dom_smoothie）
// ──────────────────────────────────────────────

/// 使用 dom_smoothie 提取文章正文（Readability 模式）
///
/// ```rust
/// let html = r#"<html><body><article><p>正文内容</p></article></body></html>"#;
/// let content = crawlkit_parser::html::extract_readable_content(html).unwrap();
/// ```
pub fn extract_readable_content(raw_html: &str) -> Result<String> {
    let cfg = Config {
        text_mode: TextMode::Markdown,
        ..Default::default()
    };

    let mut readability = dom_smoothie::Readability::new(raw_html, None, Some(cfg))
        .map_err(|e| CrawlError::Readability(e.to_string()))?;

    let article = readability
        .parse()
        .map_err(|e| CrawlError::Readability(e.to_string()))?;

    Ok(article.content.to_string())
}

/// 使用 CSS 选择器提取文章正文
///
/// ```rust
/// let html = r#"<html><body><div class="content">正文</div></body></html>"#;
/// let content = crawlkit_parser::html::extract_content_by_selector(html, "div.content").unwrap();
/// ```
pub fn extract_content_by_selector(raw_html: &str, content_selector: &str) -> Result<String> {
    let document = Html::parse_document(raw_html);
    let selector = Selector::parse(content_selector).map_err(|e| CrawlError::Selector {
        selector: content_selector.to_string(),
        message: e.to_string(),
    })?;

    let content = document
        .select(&selector)
        .next()
        .map(|el| el.text().collect::<Vec<_>>().join("\n").trim().to_string())
        .unwrap_or_default();

    Ok(content)
}

/// 智能提取：优先 Readability，失败则回退到 CSS 选择器
///
/// ```rust
/// let html = r#"<html><body><article><p>正文</p></article></body></html>"#;
/// let content = crawlkit_parser::html::extract_content(html, "article").unwrap();
/// ```
pub fn extract_content(raw_html: &str, content_selector: &str) -> Result<String> {
    match extract_readable_content(raw_html) {
        Ok(content) if !content.is_empty() && content.len() > 100 => Ok(content),
        _ => extract_content_by_selector(raw_html, content_selector),
    }
}

/// 提取匹配指定选择器的所有文本内容
///
/// ```rust
/// let html = r#"<html><body><p>段落1</p><p>段落2</p></body></html>"#;
/// let texts = crawlkit_parser::html::extract_texts(html, "p").unwrap();
/// ```
pub fn extract_texts(raw_html: &str, selector: &str) -> Result<Vec<String>> {
    let document = Html::parse_document(raw_html);
    let sel = Selector::parse(selector).map_err(|e| CrawlError::Selector {
        selector: selector.to_string(),
        message: e.to_string(),
    })?;

    let texts: Vec<String> = document
        .select(&sel)
        .filter_map(|el| {
            let text: String = el.text().collect::<Vec<_>>().join("").trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .collect();

    Ok(texts)
}

/// 提取匹配选择器的元素的指定属性值
pub fn extract_attributes(raw_html: &str, selector: &str, attr: &str) -> Result<Vec<String>> {
    let document = Html::parse_document(raw_html);
    let sel = Selector::parse(selector).map_err(|e| CrawlError::Selector {
        selector: selector.to_string(),
        message: e.to_string(),
    })?;

    let values: Vec<String> = document
        .select(&sel)
        .filter_map(|el| el.value().attr(attr).map(|v| v.to_string()))
        .filter(|v| !v.is_empty())
        .collect();

    Ok(values)
}

/// 将 HTML 元素转为纯文本（保留段落分隔）
fn element_to_text(element: &scraper::ElementRef) -> String {
    let mut result = String::new();
    for text_piece in element.text() {
        let t = text_piece.trim();
        if !t.is_empty() {
            result.push_str(t);
            result.push('\n');
        }
    }
    let lines: Vec<&str> = result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_html() -> &'static str {
        r##"<!DOCTYPE html>
<html>
<body>
  <a class="link" href="https://example.com/article/1">文章1</a>
  <a class="link" href="/article/2">文章2</a>
  <a class="link" href="//cdn.example.com/article/3">文章3</a>
  <a class="link" href="#top">锚点</a>
  <a class="link" href="javascript:void(0)">脚本</a>
  <a class="link" href="mailto:test@example.com">邮箱</a>
</body>
</html>"##
    }

    #[test]
    fn try_extract_links_returns_selector_error() {
        let result = try_extract_links("<html></html>", "!!!invalid");
        assert!(result.is_err());
    }

    #[test]
    fn extract_absolute_links_by_css_filters_and_resolves() {
        let links = extract_absolute_links(
            sample_html(),
            "a.link",
            LinkSelectorType::Css,
            "https://example.com/base/",
        )
        .unwrap();

        assert_eq!(links.len(), 3);
        assert!(links.contains(&"https://example.com/article/1".to_string()));
        assert!(links.contains(&"https://example.com/article/2".to_string()));
        assert!(links.contains(&"https://cdn.example.com/article/3".to_string()));
    }

    #[test]
    fn extract_absolute_links_by_xpath_reads_href_attributes() {
        let links = extract_absolute_links(
            sample_html(),
            "//a[@class='link']/@href",
            LinkSelectorType::Xpath,
            "https://example.com",
        )
        .unwrap();

        assert_eq!(links.len(), 3);
        assert!(links.contains(&"https://example.com/article/1".to_string()));
        assert!(links.contains(&"https://example.com/article/2".to_string()));
        assert!(links.contains(&"https://cdn.example.com/article/3".to_string()));
    }

    #[test]
    fn resolve_url_skips_non_page_links() {
        assert!(resolve_url("https://example.com", "javascript:void(0)").is_none());
        assert!(resolve_url("https://example.com", "mailto:test@example.com").is_none());
        assert!(resolve_url("https://example.com", "#top").is_none());
    }
}
