use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("Failed to parse HTML: {0}")]
    ParseError(String),

    #[error("无效 CSS 选择器: {0}")]
    SelectorError(String),

    #[error("URL 错误: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("编码错误: {0}")]
    EncodingError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),
}

pub type ParserResult<T> = Result<T, ParserError>;

// ============================================================================
// 文本内容
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextContent {
    pub raw_text: String,
    pub cleaned_text: String,
    pub word_count: usize,
    pub char_count: usize,
    pub language: Option<String>,
    pub readability_score: Option<f64>,
    pub reading_time_minutes: Option<f64>,
}

impl TextContent {
    pub fn from_raw(raw: &str) -> Self {
        let cleaned = normalize_whitespace(raw);
        let word_count = cleaned.split_whitespace().count();
        let char_count = cleaned.chars().count();
        let reading_time = if word_count > 0 {
            Some(word_count as f64 / 225.0)
        } else {
            None
        };
        Self {
            raw_text: raw.to_string(),
            cleaned_text: cleaned,
            word_count,
            char_count,
            language: None,
            readability_score: None,
            reading_time_minutes: reading_time,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.word_count == 0
    }

    pub fn is_substantial(&self) -> bool {
        self.word_count >= 50
    }
}

// ============================================================================
// 标题
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

impl Heading {
    pub fn new(level: u8, text: impl Into<String>) -> Self {
        Self {
            level: level.clamp(1, 6),
            text: text.into(),
            id: None,
            classes: Vec::new(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

// ============================================================================
// 链接
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkRel {
    Follow,
    NoFollow,
    Ugc,
    Sponsored,
    External,
    NoOpener,
    NoReferrer,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Internal,
    External,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub href: String,
    pub url: Option<String>,
    pub text: String,
    pub title: Option<String>,
    pub rel: Vec<LinkRel>,
    pub link_type: LinkType,
    pub is_nofollow: bool,
    pub target: Option<String>,
    pub hreflang: Option<String>,
}

impl Link {
    pub fn new(href: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            url: None,
            text: text.into(),
            title: None,
            rel: Vec::new(),
            link_type: LinkType::Unknown,
            is_nofollow: false,
            target: None,
            hreflang: None,
        }
    }

    pub fn should_follow(&self) -> bool {
        !self.is_nofollow
            && !self.rel.contains(&LinkRel::Sponsored)
            && !self.rel.contains(&LinkRel::Ugc)
    }

    pub fn opens_new_tab(&self) -> bool {
        self.target.as_deref() == Some("_blank")
    }
}

// ============================================================================
// 图片
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageLoading {
    #[default]
    Eager,
    Lazy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub src: String,
    pub url: Option<String>,
    pub alt: String,
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub srcset: Option<String>,
    pub sizes: Option<String>,
    pub loading: ImageLoading,
    pub is_decorative: bool,
}

impl Image {
    pub fn new(src: impl Into<String>, alt: impl Into<String>) -> Self {
        let alt_str = alt.into();
        let is_decorative = alt_str.is_empty();
        Self {
            src: src.into(),
            url: None,
            alt: alt_str,
            title: None,
            width: None,
            height: None,
            srcset: None,
            sizes: None,
            loading: ImageLoading::default(),
            is_decorative,
        }
    }

    pub fn is_responsive(&self) -> bool {
        self.srcset.is_some()
    }
}

// ============================================================================
// 列表
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListType {
    Ordered,
    Unordered,
    Definition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub text: String,
    pub nested: Option<Box<ListContent>>,
}

impl ListItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            nested: None,
        }
    }

    pub fn with_nested(text: impl Into<String>, nested: ListContent) -> Self {
        Self {
            text: text.into(),
            nested: Some(Box::new(nested)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListContent {
    pub list_type: ListType,
    pub items: Vec<ListItem>,
    pub total_items: usize,
}

impl ListContent {
    pub fn new(list_type: ListType) -> Self {
        Self {
            list_type,
            items: Vec::new(),
            total_items: 0,
        }
    }

    pub fn add_item(&mut self, item: ListItem) {
        self.total_items += 1;
        if let Some(ref nested) = item.nested {
            self.total_items += nested.total_items;
        }
        self.items.push(item);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ============================================================================
// 表格
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub content: String,
    pub is_header: bool,
    pub colspan: u32,
    pub rowspan: u32,
}

impl TableCell {
    pub fn data(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_header: false,
            colspan: 1,
            rowspan: 1,
        }
    }

    pub fn header(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_header: true,
            colspan: 1,
            rowspan: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub is_header_row: bool,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        let is_header = cells.iter().all(|c| c.is_header);
        Self {
            cells,
            is_header_row: is_header,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableContent {
    pub caption: Option<String>,
    pub headers: Vec<TableRow>,
    pub rows: Vec<TableRow>,
    pub column_count: usize,
    pub summary: Option<String>,
}

impl TableContent {
    pub fn new() -> Self {
        Self {
            caption: None,
            headers: Vec::new(),
            rows: Vec::new(),
            column_count: 0,
            summary: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty() && self.rows.is_empty()
    }

    pub fn row_count(&self) -> usize {
        self.headers.len() + self.rows.len()
    }
}

impl Default for TableContent {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 代码块
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub code: String,
    pub language: Option<String>,
    pub line_count: usize,
    pub is_inline: bool,
    pub filename: Option<String>,
}

impl CodeBlock {
    pub fn new(code: impl Into<String>) -> Self {
        let code_str = code.into();
        let line_count = code_str.lines().count();
        Self {
            code: code_str,
            language: None,
            line_count,
            is_inline: false,
            filename: None,
        }
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    pub fn inline(mut self) -> Self {
        self.is_inline = true;
        self
    }
}

// ============================================================================
// 引用
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub text: String,
    pub cite: Option<String>,
    pub cite_url: Option<String>,
}

impl Quote {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cite: None,
            cite_url: None,
        }
    }

    pub fn with_cite(mut self, cite: impl Into<String>) -> Self {
        self.cite = Some(cite.into());
        self
    }
}

// ============================================================================
// 元数据类型
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenGraph {
    pub title: Option<String>,
    pub og_type: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub site_name: Option<String>,
    pub locale: Option<String>,
    pub video: Option<String>,
    pub audio: Option<String>,
    pub extra: HashMap<String, String>,
}

impl OpenGraph {
    pub fn is_present(&self) -> bool {
        self.title.is_some() || self.og_type.is_some() || self.url.is_some()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TwitterCard {
    pub card: Option<String>,
    pub site: Option<String>,
    pub creator: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub extra: HashMap<String, String>,
}

impl TwitterCard {
    pub fn is_present(&self) -> bool {
        self.card.is_some() || self.site.is_some()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotsMeta {
    pub index: bool,
    pub follow: bool,
    pub archive: bool,
    pub cache: bool,
    pub snippet: bool,
    pub max_snippet: i32,
    pub max_image_preview: Option<String>,
    pub max_video_preview: i32,
    pub raw: Option<String>,
}

impl RobotsMeta {
    pub fn allowed() -> Self {
        Self {
            index: true,
            follow: true,
            archive: true,
            cache: true,
            snippet: true,
            max_snippet: -1,
            max_image_preview: Some("large".to_string()),
            max_video_preview: -1,
            raw: None,
        }
    }

    pub fn noindex_nofollow() -> Self {
        Self {
            index: false,
            follow: false,
            ..Self::allowed()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternateLink {
    pub hreflang: String,
    pub href: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub author: Option<String>,
    pub generator: Option<String>,
    pub canonical: Option<String>,
    pub base_url: Option<String>,
    pub language: Option<String>,
    pub charset: Option<String>,
    pub viewport: Option<String>,
    pub robots: RobotsMeta,
    pub opengraph: OpenGraph,
    pub twitter: TwitterCard,
    pub alternates: Vec<AlternateLink>,
    pub favicon: Option<String>,
    pub apple_touch_icon: Option<String>,
    pub theme_color: Option<String>,
    pub published_date: Option<String>,
    pub modified_date: Option<String>,
    pub schema_type: Option<String>,
    pub custom: HashMap<String, String>,
}

impl PageMetadata {
    pub fn effective_title(&self) -> Option<&str> {
        self.opengraph.title.as_deref()
            .or(self.twitter.title.as_deref())
            .or(self.title.as_deref())
    }

    pub fn effective_description(&self) -> Option<&str> {
        self.opengraph.description.as_deref()
            .or(self.twitter.description.as_deref())
            .or(self.description.as_deref())
    }

    pub fn effective_image(&self) -> Option<&str> {
        self.opengraph.image.as_deref()
            .or(self.twitter.image.as_deref())
    }

    pub fn should_index(&self) -> bool {
        self.robots.index
    }

    pub fn should_follow(&self) -> bool {
        self.robots.follow
    }
}

// ============================================================================
// 结构化数据
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredDataFormat {
    JsonLd,
    Microdata,
    Rdfa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredData {
    pub format: StructuredDataFormat,
    pub schema_type: Option<String>,
    pub raw_json: Option<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

impl StructuredData {
    pub fn json_ld(raw: impl Into<String>) -> Self {
        Self {
            format: StructuredDataFormat::JsonLd,
            schema_type: None,
            raw_json: Some(raw.into()),
            properties: HashMap::new(),
        }
    }

    pub fn microdata(schema_type: impl Into<String>) -> Self {
        Self {
            format: StructuredDataFormat::Microdata,
            schema_type: Some(schema_type.into()),
            raw_json: None,
            properties: HashMap::new(),
        }
    }
}

// ============================================================================
// 完整解析结果
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedContent {
    pub metadata: PageMetadata,
    pub text: TextContent,
    pub headings: Vec<Heading>,
    pub paragraphs: Vec<String>,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub lists: Vec<ListContent>,
    pub tables: Vec<TableContent>,
    pub code_blocks: Vec<CodeBlock>,
    pub quotes: Vec<Quote>,
    pub structured_data: Vec<StructuredData>,
    pub stats: ParseStats,
}

impl ParsedContent {
    pub fn internal_links(&self) -> Vec<&Link> {
        self.links.iter().filter(|l| l.link_type == LinkType::Internal).collect()
    }

    pub fn external_links(&self) -> Vec<&Link> {
        self.links.iter().filter(|l| l.link_type == LinkType::External).collect()
    }

    pub fn followable_links(&self) -> Vec<&Link> {
        self.links.iter().filter(|l| l.should_follow()).collect()
    }

    pub fn has_structured_data(&self) -> bool {
        !self.structured_data.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseStats {
    pub html_size: usize,
    pub parse_time_us: u64,
    pub node_count: usize,
    pub element_count: usize,
    pub text_node_count: usize,
    pub comment_count: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ParseStats {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

// ============================================================================
// 解析器配置
// ============================================================================

#[derive(Debug, Clone)]
pub struct ParserConfig {
    pub base_url: Option<url::Url>,
    pub max_text_length: usize,
    pub extract_images: bool,
    pub extract_links: bool,
    pub extract_tables: bool,
    pub extract_code_blocks: bool,
    pub extract_structured_data: bool,
    pub compute_readability: bool,
    pub min_paragraph_length: usize,
    pub content_selectors: Vec<String>,
    pub remove_selectors: Vec<String>,
    pub preserve_whitespace: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            max_text_length: 1_000_000,
            extract_images: true,
            extract_links: true,
            extract_tables: true,
            extract_code_blocks: true,
            extract_structured_data: true,
            compute_readability: false,
            min_paragraph_length: 20,
            content_selectors: vec![
                "article".to_string(),
                "main".to_string(),
                "[role=main]".to_string(),
                ".content".to_string(),
                ".post-content".to_string(),
                ".entry-content".to_string(),
            ],
            remove_selectors: vec![
                "script".to_string(),
                "style".to_string(),
                "noscript".to_string(),
                "nav".to_string(),
                "header".to_string(),
                "footer".to_string(),
                "aside".to_string(),
                ".sidebar".to_string(),
                ".advertisement".to_string(),
                ".ad".to_string(),
                ".ads".to_string(),
                "[role=navigation]".to_string(),
                "[role=banner]".to_string(),
                "[role=contentinfo]".to_string(),
            ],
            preserve_whitespace: false,
        }
    }
}

impl ParserConfig {
    pub fn with_base_url(url: impl AsRef<str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            base_url: Some(url::Url::parse(url.as_ref())?),
            ..Default::default()
        })
    }

    pub fn minimal() -> Self {
        Self {
            extract_images: false,
            extract_tables: false,
            extract_code_blocks: false,
            extract_structured_data: false,
            compute_readability: false,
            ..Default::default()
        }
    }

    pub fn full() -> Self {
        Self {
            compute_readability: true,
            ..Default::default()
        }
    }

    pub fn base_url(mut self, url: url::Url) -> Self {
        self.base_url = Some(url);
        self
    }

    pub fn add_content_selector(mut self, selector: impl Into<String>) -> Self {
        self.content_selectors.push(selector.into());
        self
    }

    pub fn add_remove_selector(mut self, selector: impl Into<String>) -> Self {
        self.remove_selectors.push(selector.into());
        self
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

pub fn normalize_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                result.push(' ');
                prev_ws = true;
            }
        } else {
            result.push(c);
            prev_ws = false;
        }
    }
    result.trim().to_string()
}

pub fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect()
}

pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        let mut truncated: String = text.chars().take(max_len - 3).collect();
        truncated.push_str("...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_content_creation() {
        let text = TextContent::from_raw("Hello   world,   this is   a test.");
        assert_eq!(text.cleaned_text, "Hello world, this is a test.");
        assert_eq!(text.word_count, 6);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_heading_creation() {
        let h1 = Heading::new(1, "Main Title").with_id("main");
        assert_eq!(h1.level, 1);
        assert_eq!(h1.id, Some("main".to_string()));
    }

    #[test]
    fn test_link_creation() {
        let link = Link::new("https://example.com", "Example");
        assert!(!link.is_nofollow);
        assert!(link.should_follow());
    }

    #[test]
    fn test_image_creation() {
        let img = Image::new("/img/photo.jpg", "A photo");
        assert!(!img.is_decorative);
        let decorative = Image::new("/img/spacer.gif", "");
        assert!(decorative.is_decorative);
    }

    #[test]
    fn test_opengraph() {
        let og = OpenGraph::default();
        assert!(!og.is_present());
        let og2 = OpenGraph {
            title: Some("Test".to_string()),
            ..Default::default()
        };
        assert!(og2.is_present());
    }

    #[test]
    fn test_parser_config() {
        let config = ParserConfig::default();
        assert!(config.extract_images);
        let minimal = ParserConfig::minimal();
        assert!(!minimal.extract_images);
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("  "), "");
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("Hello", 10), "Hello");
        assert_eq!(truncate_text("Hello World", 8), "Hello...");
    }
}
