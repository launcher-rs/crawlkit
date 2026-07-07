//! # HtmlParser — 统一解析入口
//!
//! 提供 `HtmlParser` 结构体，作为所有 HTML 解析操作的中心入口。
//! 封装文档解析、元数据提取、文本提取、结构化内容提取等功能。
//! 适配自 halldyll-parser 的解析器编排层。

use scraper::Html;
use std::time::Instant;
use url::Url;

use crate::content::{
    extract_code_blocks, extract_headings, extract_images, extract_lists, extract_paragraphs,
    extract_quotes, extract_tables,
};
use crate::links::extract_links;
use crate::metadata::{extract_metadata, extract_structured_data};
use crate::text::extract_text;
use crate::types::{
    CodeBlock, Heading, Image, Link, ListContent, PageMetadata, ParseStats, ParsedContent,
    ParserConfig, ParserResult, Quote, StructuredData, TableContent, TextContent,
};

// ============================================================================
// HtmlParser
// ============================================================================

/// 统一的 HTML 解析器，封装文档解析与内容提取的全流程。
///
/// 通过 `ParserConfig` 控制提取的内容类型和行为。
///
/// # 示例
///
/// ```ignore
/// let parser = HtmlParser::new();
/// let result = parser.parse("<html><body><p>Hello</p></body></html>")?;
/// println!("{}", result.text.cleaned_text);
/// ```
#[derive(Debug, Clone)]
pub struct HtmlParser {
    config: ParserConfig,
}

impl HtmlParser {
    // ── 构造方法 ──────────────────────────────────────────────────────────

    /// 使用默认配置创建解析器。
    pub fn new() -> Self {
        Self {
            config: ParserConfig::default(),
        }
    }

    /// 使用自定义配置创建解析器。
    pub fn with_config(config: ParserConfig) -> Self {
        Self { config }
    }

    /// 使用指定的基础 URL 创建解析器。
    pub fn with_base_url(url: Url) -> Self {
        Self {
            config: ParserConfig {
                base_url: Some(url),
                ..ParserConfig::default()
            },
        }
    }

    /// 设置基础 URL，用于解析相对链接和图片地址。
    pub fn set_base_url(&mut self, url: Url) {
        self.config.base_url = Some(url);
    }

    /// 获取当前配置的引用。
    pub fn config(&self) -> &ParserConfig {
        &self.config
    }

    /// 获取当前配置的可变引用。
    pub fn config_mut(&mut self) -> &mut ParserConfig {
        &mut self.config
    }

    // ── 核心解析方法 ──────────────────────────────────────────────────────

    /// 解析完整的 HTML 文档，返回结构化内容。
    ///
    /// 自动计算解析统计信息，并根据配置决定提取哪些内容类型。
    pub fn parse(&self, html: &str) -> ParserResult<ParsedContent> {
        let start = Instant::now();
        let document = Html::parse_document(html);
        let parse_time_us = start.elapsed().as_micros() as u64;

        let html_size = html.len();
        let node_count = count_nodes(&document);

        let metadata = extract_metadata(&document, self.config.base_url.as_ref())?;
        let text = extract_text(&document, &self.config)?;
        let headings = extract_headings(&document);
        let paragraphs = extract_paragraphs(&document, self.config.min_paragraph_length);

        let links = if self.config.extract_links {
            extract_links(&document, &self.config)?
        } else {
            Vec::new()
        };

        let images = if self.config.extract_images {
            extract_images(&document, self.config.base_url.as_ref())
        } else {
            Vec::new()
        };

        let lists = extract_lists(&document, &self.config);

        let tables = if self.config.extract_tables {
            extract_tables(&document, &self.config)
        } else {
            Vec::new()
        };

        let code_blocks = if self.config.extract_code_blocks {
            extract_code_blocks(&document, &self.config)
        } else {
            Vec::new()
        };

        let quotes = extract_quotes(&document, &self.config);

        let structured_data = if self.config.extract_structured_data {
            extract_structured_data(&document)
        } else {
            Vec::new()
        };

        let stats = ParseStats {
            html_size,
            parse_time_us,
            node_count,
            element_count: count_elements(&document),
            text_node_count: count_text_nodes(&document),
            comment_count: count_comments(&document),
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        Ok(ParsedContent {
            metadata,
            text,
            headings,
            paragraphs,
            links,
            images,
            lists,
            tables,
            code_blocks,
            quotes,
            structured_data,
            stats,
        })
    }

    /// 解析 HTML 片段（不包含 `<html>`/`<body>` 包装）。
    ///
    /// 适用于解析页面中的局部 HTML 内容。
    pub fn parse_fragment(&self, html: &str) -> ParserResult<ParsedContent> {
        let start = Instant::now();
        let document = Html::parse_fragment(html);
        let parse_time_us = start.elapsed().as_micros() as u64;

        let html_size = html.len();
        let metadata = PageMetadata::default();
        let text = extract_text(&document, &self.config)?;
        let headings = extract_headings(&document);
        let paragraphs = extract_paragraphs(&document, self.config.min_paragraph_length);

        let links = if self.config.extract_links {
            extract_links(&document, &self.config)?
        } else {
            Vec::new()
        };

        let images = if self.config.extract_images {
            extract_images(&document, self.config.base_url.as_ref())
        } else {
            Vec::new()
        };

        let lists = extract_lists(&document, &self.config);

        let tables = if self.config.extract_tables {
            extract_tables(&document, &self.config)
        } else {
            Vec::new()
        };

        let code_blocks = if self.config.extract_code_blocks {
            extract_code_blocks(&document, &self.config)
        } else {
            Vec::new()
        };

        let quotes = extract_quotes(&document, &self.config);

        let stats = ParseStats {
            html_size,
            parse_time_us,
            node_count: count_nodes(&document),
            element_count: count_elements(&document),
            text_node_count: count_text_nodes(&document),
            comment_count: count_comments(&document),
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        Ok(ParsedContent {
            metadata,
            text,
            headings,
            paragraphs,
            links,
            images,
            lists,
            tables,
            code_blocks,
            quotes,
            structured_data: Vec::new(),
            stats,
        })
    }

    // ── 单项提取方法 ──────────────────────────────────────────────────────

    /// 提取页面元数据（标题、描述、OG、Twitter Card、结构化数据等）。
    pub fn extract_metadata(&self, html: &str) -> ParserResult<PageMetadata> {
        let document = Html::parse_document(html);
        extract_metadata(&document, self.config.base_url.as_ref())
    }

    /// 提取页面文本内容，含清洗、可读性评分。
    pub fn extract_text(&self, html: &str) -> ParserResult<TextContent> {
        let document = Html::parse_document(html);
        extract_text(&document, &self.config)
    }

    /// 提取所有标题（h1 - h6）。
    pub fn extract_headings(&self, html: &str) -> Vec<Heading> {
        let document = Html::parse_document(html);
        extract_headings(&document)
    }

    /// 提取所有链接。
    pub fn extract_links(&self, html: &str) -> ParserResult<Vec<Link>> {
        let document = Html::parse_document(html);
        extract_links(&document, &self.config)
    }

    /// 提取所有图片，自动解析相对地址为绝对地址。
    pub fn extract_images(&self, html: &str) -> Vec<Image> {
        let document = Html::parse_document(html);
        extract_images(&document, self.config.base_url.as_ref())
    }

    /// 提取所有列表（有序、无序、定义列表）。
    pub fn extract_lists(&self, html: &str) -> Vec<ListContent> {
        let document = Html::parse_document(html);
        extract_lists(&document, &self.config)
    }

    /// 提取所有表格。
    pub fn extract_tables(&self, html: &str) -> Vec<TableContent> {
        let document = Html::parse_document(html);
        extract_tables(&document, &self.config)
    }

    /// 提取所有代码块。
    pub fn extract_code_blocks(&self, html: &str) -> Vec<CodeBlock> {
        let document = Html::parse_document(html);
        extract_code_blocks(&document, &self.config)
    }

    /// 提取所有引用（blockquote）。
    pub fn extract_quotes(&self, html: &str) -> Vec<Quote> {
        let document = Html::parse_document(html);
        extract_quotes(&document, &self.config)
    }

    /// 提取所有结构化数据（JSON-LD + Microdata）。
    pub fn extract_structured_data(&self, html: &str) -> Vec<StructuredData> {
        let document = Html::parse_document(html);
        extract_structured_data(&document)
    }

    // ── URL 工具方法 ──────────────────────────────────────────────────────

    /// 将相对路径或协议相对 URL 解析为绝对 URL。
    ///
    /// 需要解析器已配置 `base_url`，否则返回 `None`。
    pub fn resolve_url(&self, href: &str) -> Option<String> {
        let base = self.config.base_url.as_ref()?;
        resolve_relative_url(base, href)
    }

    /// 检查是否已配置基础 URL。
    pub fn has_base_url(&self) -> bool {
        self.config.base_url.is_some()
    }

    /// 获取当前基础 URL 的引用。
    pub fn base_url(&self) -> Option<&Url> {
        self.config.base_url.as_ref()
    }
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 使用默认配置快速解析 HTML 文档。
pub fn parse(html: &str) -> ParserResult<ParsedContent> {
    HtmlParser::new().parse(html)
}

/// 使用指定基础 URL 解析 HTML 文档，支持相对地址解析。
pub fn parse_with_url(html: &str, base_url: Url) -> ParserResult<ParsedContent> {
    let parser = HtmlParser::with_base_url(base_url);
    parser.parse(html)
}

/// 快速提取 HTML 页面的元数据。
pub fn get_metadata(html: &str) -> ParserResult<PageMetadata> {
    let parser = HtmlParser::new();
    parser.extract_metadata(html)
}

/// 快速提取 HTML 页面的纯文本内容。
pub fn get_text(html: &str) -> ParserResult<TextContent> {
    let parser = HtmlParser::new();
    parser.extract_text(html)
}

/// 快速提取 HTML 页面中的所有链接。
pub fn get_links(html: &str) -> ParserResult<Vec<Link>> {
    let parser = HtmlParser::new();
    parser.extract_links(html)
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 将相对 URL 解析为绝对 URL。
fn resolve_relative_url(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    if href.starts_with("//") {
        let scheme = base.scheme();
        return Some(format!("{scheme}:{href}"));
    }
    base.join(href).ok().map(|u| u.to_string())
}

/// 统计文档中的节点总数。
fn count_nodes(document: &Html) -> usize {
    document.root_element().children().count()
}

/// 统计文档中的元素节点数。
fn count_elements(document: &Html) -> usize {
    count_elements_recursive(&document.root_element())
}

fn count_elements_recursive(element: &scraper::ElementRef) -> usize {
    let mut count = 1;
    for child in element.children() {
        if let Some(child_ref) = scraper::ElementRef::wrap(child) {
            count += count_elements_recursive(&child_ref);
        }
    }
    count
}

/// 统计文档中的文本节点数。
fn count_text_nodes(document: &Html) -> usize {
    count_text_nodes_recursive(&document.root_element())
}

fn count_text_nodes_recursive(element: &scraper::ElementRef) -> usize {
    let mut count = 0;
    for child in element.children() {
        match child.value() {
            scraper::Node::Text(_) => count += 1,
            _ => {}
        }
        if let Some(child_ref) = scraper::ElementRef::wrap(child) {
            count += count_text_nodes_recursive(&child_ref);
        }
    }
    count
}

/// 统计文档中的注释节点数。
fn count_comments(document: &Html) -> usize {
    count_comments_recursive(&document.root_element())
}

fn count_comments_recursive(element: &scraper::ElementRef) -> usize {
    let mut count = 0;
    for child in element.children() {
        if matches!(child.value(), scraper::Node::Comment(_)) {
            count += 1;
        }
        if let Some(child_ref) = scraper::ElementRef::wrap(child) {
            count += count_comments_recursive(&child_ref);
        }
    }
    count
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 包含各类 HTML 结构的示例页面，用于集成测试。
    const SAMPLE_HTML: &str = r##"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="utf-8">
    <title>示例页面 — 测试用</title>
    <meta name="description" content="这是一个用于解析测试的示例页面">
    <meta name="keywords" content="测试, html, 解析">
    <meta name="author" content="测试作者">
    <meta property="og:title" content="OG 示例标题">
    <meta property="og:type" content="article">
    <meta name="twitter:card" content="summary">
    <link rel="canonical" href="https://example.com/page">
    <link rel="icon" href="/favicon.ico">
    <base href="https://example.com/blog/">
</head>
<body>
    <h1 id="main-title">主标题</h1>
    <h2 class="section">第一章</h2>
    <p>这是第一段文字。包含足够长的内容来满足最小段落长度要求。</p>
    <p>这是第二段文字，同样有足够的内容来通过过滤器。</p>

    <ul>
        <li>无序列表项一</li>
        <li>无序列表项二</li>
        <li>无序列表项三</li>
    </ul>

    <ol>
        <li>有序第一</li>
        <li>有序第二</li>
    </ol>

    <table>
        <thead>
            <tr><th>姓名</th><th>年龄</th></tr>
        </thead>
        <tbody>
            <tr><td>张三</td><td>28</td></tr>
            <tr><td>李四</td><td>35</td></tr>
        </tbody>
    </table>

    <pre><code class="language-rust">fn hello() { println!("你好"); }</code></pre>

    <blockquote cite="https://example.com/quote">
        这是一段引用文字。
        <cite>作者名</cite>
    </blockquote>

    <img src="/images/photo.jpg" alt="示例图片" width="800" height="600">

    <a href="https://example.com/other">外部链接</a>
    <a href="/relative-page">相对链接</a>
    <a href="mailto:test@example.com">邮箱链接</a>

    <script type="application/ld+json">
        {"@type": "Article", "name": "示例文章", "description": "测试"}
    </script>
</body>
</html>
"##;

    // ── 构造方法 ──

    #[test]
    fn 默认构造() {
        let parser = HtmlParser::new();
        assert!(parser.config().base_url.is_none());
    }

    #[test]
    fn 带配置构造() {
        let config = ParserConfig::minimal();
        let parser = HtmlParser::with_config(config);
        assert!(!parser.config().extract_images);
    }

    #[test]
    fn 带基础URL构造() {
        let url = Url::parse("https://example.com").unwrap();
        let parser = HtmlParser::with_base_url(url);
        assert!(parser.has_base_url());
    }

    #[test]
    fn 设置基础URL() {
        let mut parser = HtmlParser::new();
        assert!(!parser.has_base_url());
        parser.set_base_url(Url::parse("https://example.com").unwrap());
        assert!(parser.has_base_url());
    }

    #[test]
    fn 获取配置引用() {
        let parser = HtmlParser::new();
        let config = parser.config();
        assert!(config.extract_images);
    }

    #[test]
    fn 获取配置可变引用() {
        let mut parser = HtmlParser::new();
        parser.config_mut().extract_images = false;
        assert!(!parser.config().extract_images);
    }

    #[test]
    fn 默认trait实现() {
        let parser: HtmlParser = Default::default();
        assert!(parser.config().extract_images);
    }

    // ── 完整解析 ──

    #[test]
    fn 解析完整文档() {
        let parser = HtmlParser::new();
        let result = parser.parse(SAMPLE_HTML).unwrap();
        assert!(result.metadata.title.is_some());
        assert!(result.text.word_count > 0);
        assert!(!result.headings.is_empty());
        assert!(!result.paragraphs.is_empty());
        assert!(!result.links.is_empty());
        assert!(!result.images.is_empty());
        assert!(!result.lists.is_empty());
        assert!(!result.tables.is_empty());
        assert!(!result.code_blocks.is_empty());
        assert!(!result.quotes.is_empty());
        assert!(!result.structured_data.is_empty());
    }

    #[test]
    fn 解析文档统计信息() {
        let parser = HtmlParser::new();
        let result = parser.parse(SAMPLE_HTML).unwrap();
        assert!(result.stats.html_size > 0);
        assert!(result.stats.parse_time_us > 0);
        assert!(result.stats.node_count > 0);
    }

    #[test]
    fn 解析空文档() {
        let parser = HtmlParser::new();
        let result = parser.parse("").unwrap();
        assert_eq!(result.text.word_count, 0);
        assert!(result.headings.is_empty());
    }

    #[test]
    fn 解析最小化文档() {
        let parser = HtmlParser::with_config(ParserConfig::minimal());
        let result = parser.parse(SAMPLE_HTML).unwrap();
        assert!(result.images.is_empty());
        assert!(result.tables.is_empty());
        assert!(result.code_blocks.is_empty());
        assert!(result.structured_data.is_empty());
        assert!(result.metadata.title.is_some());
    }

    // ── 片段解析 ──

    #[test]
    fn 解析HTML片段() {
        let parser = HtmlParser::new();
        let fragment = "<p>这是片段内容。</p><a href=\"https://example.com\">链接</a>";
        let result = parser.parse_fragment(fragment).unwrap();
        assert!(result.text.cleaned_text.contains("片段内容"));
        assert_eq!(result.links.len(), 1);
    }

    #[test]
    fn 片段解析无元数据() {
        let parser = HtmlParser::new();
        let result = parser.parse_fragment("<p>纯文本</p>").unwrap();
        assert!(result.metadata.title.is_none());
    }

    // ── 单项提取 ──

    #[test]
    fn 单项提取元数据() {
        let parser = HtmlParser::new();
        let meta = parser.extract_metadata(SAMPLE_HTML).unwrap();
        assert_eq!(meta.title.as_deref(), Some("OG 示例标题"));
        assert!(meta.description.is_some());
        assert!(!meta.keywords.is_empty());
    }

    #[test]
    fn 单项提取文本() {
        let parser = HtmlParser::new();
        let text = parser.extract_text(SAMPLE_HTML).unwrap();
        assert!(text.cleaned_text.contains("主标题"));
        assert!(text.word_count > 5);
    }

    #[test]
    fn 单项提取标题() {
        let parser = HtmlParser::new();
        let headings = parser.extract_headings(SAMPLE_HTML);
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "主标题");
        assert_eq!(headings[1].text, "第一章");
    }

    #[test]
    fn 单项提取链接() {
        let parser = HtmlParser::new();
        let links = parser.extract_links(SAMPLE_HTML).unwrap();
        assert!(!links.is_empty());
        let has_external = links.iter().any(|l| l.href.contains("example.com/other"));
        assert!(has_external);
    }

    #[test]
    fn 单项提取图片() {
        let parser = HtmlParser::new();
        let images = parser.extract_images(SAMPLE_HTML);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].alt, "示例图片");
    }

    #[test]
    fn 单项提取列表() {
        let parser = HtmlParser::new();
        let lists = parser.extract_lists(SAMPLE_HTML);
        assert_eq!(lists.len(), 2);
        let ul = lists.iter().find(|l| l.items.len() == 3);
        assert!(ul.is_some());
    }

    #[test]
    fn 单项提取表格() {
        let parser = HtmlParser::new();
        let tables = parser.extract_tables(SAMPLE_HTML);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].rows.len(), 2);
    }

    #[test]
    fn 单项提取代码块() {
        let parser = HtmlParser::new();
        let blocks = parser.extract_code_blocks(SAMPLE_HTML);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language.as_deref(), Some("rust"));
    }

    #[test]
    fn 单项提取引用() {
        let parser = HtmlParser::new();
        let quotes = parser.extract_quotes(SAMPLE_HTML);
        assert_eq!(quotes.len(), 1);
    }

    #[test]
    fn 单项提取结构化数据() {
        let parser = HtmlParser::new();
        let data = parser.extract_structured_data(SAMPLE_HTML);
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].schema_type.as_deref(), Some("Article"));
    }

    // ── URL 工具 ──

    #[test]
    fn 解析相对URL() {
        let parser = HtmlParser::with_base_url(Url::parse("https://example.com").unwrap());
        let resolved = parser.resolve_url("/path/to/page");
        assert_eq!(
            resolved.as_deref(),
            Some("https://example.com/path/to/page")
        );
    }

    #[test]
    fn 解析绝对URL() {
        let parser = HtmlParser::with_base_url(Url::parse("https://example.com").unwrap());
        let resolved = parser.resolve_url("https://other.com/page");
        assert_eq!(resolved.as_deref(), Some("https://other.com/page"));
    }

    #[test]
    fn 无基础URL时返回None() {
        let parser = HtmlParser::new();
        assert!(parser.resolve_url("/page").is_none());
    }

    #[test]
    fn 空href返回None() {
        let parser = HtmlParser::with_base_url(Url::parse("https://example.com").unwrap());
        assert!(parser.resolve_url("").is_none());
    }

    #[test]
    fn 检查是否有基础URL() {
        let parser = HtmlParser::new();
        assert!(!parser.has_base_url());
        let parser2 = HtmlParser::with_base_url(Url::parse("https://example.com").unwrap());
        assert!(parser2.has_base_url());
    }

    #[test]
    fn 获取基础URL引用() {
        let url = Url::parse("https://example.com").unwrap();
        let parser = HtmlParser::with_base_url(url.clone());
        assert_eq!(parser.base_url(), Some(&url));
    }

    // ── 便捷函数 ──

    #[test]
    fn 便捷解析函数() {
        let result = parse(SAMPLE_HTML).unwrap();
        assert!(result.metadata.title.is_some());
    }

    #[test]
    fn 便捷解析带URL() {
        let url = Url::parse("https://example.com").unwrap();
        let result = parse_with_url(SAMPLE_HTML, url).unwrap();
        assert!(result.links.iter().any(|l| l.url.is_some()));
    }

    #[test]
    fn 便捷获取元数据() {
        let meta = get_metadata(SAMPLE_HTML).unwrap();
        assert_eq!(meta.author.as_deref(), Some("测试作者"));
    }

    #[test]
    fn 便捷获取文本() {
        let text = get_text(SAMPLE_HTML).unwrap();
        assert!(text.word_count > 0);
    }

    #[test]
    fn 便捷获取链接() {
        let links = get_links(SAMPLE_HTML).unwrap();
        assert!(!links.is_empty());
    }

    // ── resolve_relative_url 辅助函数 ──

    #[test]
    fn 内部resolve相对路径() {
        let base = Url::parse("https://example.com/base/").unwrap();
        assert_eq!(
            resolve_relative_url(&base, "page.html"),
            Some("https://example.com/base/page.html".to_string())
        );
    }

    #[test]
    fn 内部resolve根相对路径() {
        let base = Url::parse("https://example.com/base/").unwrap();
        assert_eq!(
            resolve_relative_url(&base, "/page.html"),
            Some("https://example.com/page.html".to_string())
        );
    }

    #[test]
    fn 内部resolve协议相对() {
        let base = Url::parse("https://example.com").unwrap();
        assert_eq!(
            resolve_relative_url(&base, "//cdn.example.com/file.js"),
            Some("https://cdn.example.com/file.js".to_string())
        );
    }

    #[test]
    fn 内部resolve跳过空白() {
        let base = Url::parse("https://example.com").unwrap();
        assert!(resolve_relative_url(&base, "").is_none());
        assert!(resolve_relative_url(&base, "  ").is_none());
    }

    // ── 节点统计 ──

    #[test]
    fn 统计节点数() {
        let doc = Html::parse_document("<html><body><p>文本</p></body></html>");
        assert!(count_nodes(&doc) > 0);
    }

    #[test]
    fn 统计元素数() {
        let doc = Html::parse_document(
            "<html><head></head><body><p>文本</p><div><span>内联</span></div></body></html>",
        );
        let count = count_elements(&doc);
        // html, head, body, p, div, span = 6
        assert_eq!(count, 6);
    }

    #[test]
    fn 统计文本节点数() {
        let doc = Html::parse_document("<html><body><p>Hello</p><p>World</p></body></html>");
        let count = count_text_nodes(&doc);
        assert_eq!(count, 2);
    }

    #[test]
    fn 统计注释数() {
        let doc = Html::parse_document(
            "<html><body><!-- 注释一 --><p>文本</p><!-- 注释二 --></body></html>",
        );
        let count = count_comments(&doc);
        assert_eq!(count, 2);
    }

    // ── 边界情况 ──

    #[test]
    fn 解析包含特殊字符的HTML() {
        let html = "<html><body><p>&lt;script&gt;alert('xss')&lt;/script&gt;</p></body></html>";
        let parser = HtmlParser::new();
        let result = parser.parse(html).unwrap();
        assert!(result.text.cleaned_text.contains("alert('xss')"));
    }

    #[test]
    fn 解析多语言混合内容() {
        let html = "<html><body><p>Hello 世界 こんにちは</p></body></html>";
        let parser = HtmlParser::new();
        let result = parser.parse(html).unwrap();
        assert!(result.text.cleaned_text.contains("Hello 世界"));
    }

    #[test]
    fn 解析极大段落不崩溃() {
        let long_text = "A ".repeat(10_000);
        let html = format!("<html><body><p>{}</p></body></html>", long_text);
        let parser = HtmlParser::new();
        let result = parser.parse(&html).unwrap();
        assert!(result.text.word_count > 0);
    }

    #[test]
    fn 配置关闭所有提取() {
        let config = ParserConfig {
            extract_images: false,
            extract_links: false,
            extract_tables: false,
            extract_code_blocks: false,
            extract_structured_data: false,
            ..ParserConfig::default()
        };
        let parser = HtmlParser::with_config(config);
        let result = parser.parse(SAMPLE_HTML).unwrap();
        assert!(result.images.is_empty());
        assert!(result.tables.is_empty());
        assert!(result.code_blocks.is_empty());
        assert!(result.structured_data.is_empty());
    }

    #[test]
    fn 基础URL解析图片地址() {
        let url = Url::parse("https://example.com").unwrap();
        let parser = HtmlParser::with_base_url(url);
        let result = parser.parse(SAMPLE_HTML).unwrap();
        let img = &result.images[0];
        assert_eq!(
            img.url.as_deref(),
            Some("https://example.com/images/photo.jpg")
        );
    }
}
