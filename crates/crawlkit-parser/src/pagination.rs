//! 分页检测与提取模块
//!
//! 基于 halldyll-parser 的分页逻辑改写。
//! 提供从 HTML 文档中检测分页类型、提取分页链接、解析页码等功能，
//! 支持数字分页、上一页/下一页、无限滚动、加载更多、游标与偏移量等多种分页模式。

use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

// ============================================================================
// 数据结构定义
// ============================================================================

/// 分页链接
///
/// 表示分页导航中的一个具体页面链接及其对应的页码。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageUrl {
    /// 页面 URL
    pub url: String,
    /// 页码（从 1 开始）
    pub page_number: u32,
    /// 是否为当前页
    pub is_current: bool,
}

impl PageUrl {
    /// 创建新的分页链接
    pub fn new(url: impl Into<String>, page_number: u32, is_current: bool) -> Self {
        Self {
            url: url.into(),
            page_number,
            is_current,
        }
    }
}

/// 分页类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaginationType {
    /// 数字分页（1, 2, 3, …）
    Numbered,
    /// 上一页 / 下一页模式
    NextPrev,
    /// 无限滚动
    InfiniteScroll,
    /// 点击「加载更多」按钮
    LoadMore,
    /// 游标分页（after/before cursor）
    Cursor,
    /// 偏移量分页（offset/limit）
    Offset,
    /// 无分页
    None,
}

/// 分页信息
///
/// 表示从 HTML 文档中提取的完整分页导航信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// 当前页码
    pub current_page: u32,
    /// 总页数（部分分页模式可能无法获取）
    pub total_pages: Option<u32>,
    /// 上一页 URL
    pub prev_url: Option<String>,
    /// 下一页 URL
    pub next_url: Option<String>,
    /// 第一页 URL
    pub first_url: Option<String>,
    /// 最后一页 URL
    pub last_url: Option<String>,
    /// 所有分页链接列表
    pub page_urls: Vec<PageUrl>,
    /// 分页类型
    pub pagination_type: PaginationType,
    /// 是否使用无限滚动
    pub has_infinite_scroll: bool,
    /// 是否使用「加载更多」
    pub has_load_more: bool,
    /// 每页条目数（如可推断）
    pub items_per_page: Option<u32>,
    /// 总条目数（如可获取）
    pub total_items: Option<u32>,
}

impl Pagination {
    /// 创建默认的 Pagination（无分页）
    pub fn none() -> Self {
        Self {
            current_page: 1,
            total_pages: None,
            prev_url: None,
            next_url: None,
            first_url: None,
            last_url: None,
            page_urls: Vec::new(),
            pagination_type: PaginationType::None,
            has_infinite_scroll: false,
            has_load_more: false,
            items_per_page: None,
            total_items: None,
        }
    }

    /// 判断该页面是否有分页导航
    pub fn has_pagination(&self) -> bool {
        self.pagination_type != PaginationType::None
            || self.page_urls.len() > 1
            || self.next_url.is_some()
            || self.has_infinite_scroll
            || self.has_load_more
    }

    /// 返回下一页的 URL
    pub fn next_page_url(&self) -> Option<&str> {
        self.next_url.as_deref()
    }

    /// 返回上一页的 URL
    pub fn prev_page_url(&self) -> Option<&str> {
        self.prev_url.as_deref()
    }

    /// 获取所有数字分页链接（排除上一页/下一页）
    pub fn numbered_pages(&self) -> Vec<&PageUrl> {
        self.page_urls
            .iter()
            .filter(|p| !self.is_nav_link(&p.url))
            .collect()
    }

    /// 判断是否为导航链接（上一页/下一页/首页/末页）
    fn is_nav_link(&self, url: &str) -> bool {
        Some(url) == self.prev_url.as_deref()
            || Some(url) == self.next_url.as_deref()
            || Some(url) == self.first_url.as_deref()
            || Some(url) == self.last_url.as_deref()
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self::none()
    }
}

// ============================================================================
// 正则表达式模式
// ============================================================================

lazy_static::lazy_static! {
    /// 从 URL 路径中提取页码的正则
    static ref PAGE_IN_URL: Regex = Regex::new(
        r"(?i)(?:page|p|pg)[/=](\d+)"
    ).expect("PAGE_IN_URL 正则编译失败");

    /// 从 URL 路径末尾匹配页码，如 `/page/2/` 或 `/page/2`
    static ref PAGE_IN_PATH: Regex = Regex::new(
        r"(?i)/page/(\d+)/?"
    ).expect("PAGE_IN_PATH 正则编译失败");

    /// 从查询参数中提取页码，如 `?page=2` 或 `&page=3`
    static ref PAGE_IN_QUERY: Regex = Regex::new(
        r"(?i)[?&](?:page|p|pg)=(\d+)"
    ).expect("PAGE_IN_QUERY 正则编译失败");

    /// 从路径末尾匹配纯数字段，如 `/2/` 或 `/2`
    static ref NUMERIC_PATH: Regex = Regex::new(
        r"/(\d+)/?$"
    ).expect("NUMERIC_PATH 正则编译失败");

    /// 匹配文本中的页码信息，如 "Page 1 of 10"
    static ref PAGE_TEXT: Regex = Regex::new(
        r"(?i)(?:page|p[g]?)\s*(\d+)\s*(?:of|/)\s*(\d+)"
    ).expect("PAGE_TEXT 正则编译失败");

    /// 匹配简单的页码标记，如 "Page 1"
    static ref SIMPLE_PAGE_TEXT: Regex = Regex::new(
        r"(?i)(?:page|p[g]?)\s*(\d+)"
    ).expect("SIMPLE_PAGE_TEXT 正则编译失败");

    /// 匹配总数信息，如 "共 100 条结果" 或 "100 results"
    static ref TOTAL_ITEMS_TEXT: Regex = Regex::new(
        r"(?i)(?:共|total|of)\s*(\d+)\s*(?:条|个|items?|results?|页)?[条个]?[\u4e00-\u9fff]*\s*$"
    ).expect("TOTAL_ITEMS_TEXT 正则编译失败");

    /// 匹配上一页文本
    static ref PREV_TEXT: Regex = Regex::new(
        r"(?i)^\s*(?:上一页|prev|previous|«|<|‹|←|上一页|上一頁|上一篇)\s*$"
    ).expect("PREV_TEXT 正则编译失败");

    /// 匹配下一页文本
    static ref NEXT_TEXT: Regex = Regex::new(
        r"(?i)^\s*(?:下一页|next|»|>|›|→|下一页|下一頁|下一篇)\s*$"
    ).expect("NEXT_TEXT 正则编译失败");

    /// 匹配加载更多文本
    static ref LOAD_MORE_TEXT: Regex = Regex::new(
        r"(?i)(?:load\s*more|show\s*more|view\s*more|加载更多|查看更多|展开更多)"
    ).expect("LOAD_MORE_TEXT 正则编译失败");

    /// 检测无限滚动相关属性或类名
    static ref INFINITE_SCROLL_PATTERN: Regex = Regex::new(
        r"(?i)(?:infinite[\s-]?scroll|infinite[\s-]?load|endless[\s-]?(?:scroll|page))"
    ).expect("INFINITE_SCROLL_PATTERN 正则编译失败");

    /// 检测偏移量分页
    static ref OFFSET_PATTERN: Regex = Regex::new(
        r"(?i)[?&](?:offset|start|index|from)=(\d+)"
    ).expect("OFFSET_PATTERN 正则编译失败");

    /// 检测游标分页
    static ref CURSOR_PATTERN: Regex = Regex::new(
        r"(?i)[?&](?:cursor|after|before)=([^&]+)"
    ).expect("CURSOR_PATTERN 正则编译失败");
}

// ============================================================================
// 页码提取
// ============================================================================

/// 从 URL 中提取页码。
///
/// 依次尝试以下策略：
/// 1. 查询参数 `page=`、`p=`、`pg=`（不区分大小写）
/// 2. 路径匹配 `/page/N`
/// 3. 通用模式 `page/N` 或 `p/N`
/// 4. 末尾数字路径如 `/articles/2/`
///
/// # 示例
///
/// ```
/// use crawlkit_parser::pagination::extract_page_number_from_url;
/// assert_eq!(extract_page_number_from_url("https://example.com?page=3"), Some(3));
/// assert_eq!(extract_page_number_from_url("https://example.com/page/5"), Some(5));
/// assert_eq!(extract_page_number_from_url("https://example.com/articles/2/"), Some(2));
/// ```
pub fn extract_page_number_from_url(url: &str) -> Option<u32> {
    // 优先从查询参数匹配
    if let Some(caps) = PAGE_IN_QUERY.captures(url) {
        if let Ok(n) = caps[1].parse::<u32>() {
            return Some(n);
        }
    }

    // 尝试路径格式 /page/N
    if let Some(caps) = PAGE_IN_PATH.captures(url) {
        if let Ok(n) = caps[1].parse::<u32>() {
            return Some(n);
        }
    }

    // 尝试通用 page/p/pg 模式
    if let Some(caps) = PAGE_IN_URL.captures(url) {
        if let Ok(n) = caps[1].parse::<u32>() {
            return Some(n);
        }
    }

    // 尝试末尾数字路径
    if let Some(caps) = NUMERIC_PATH.captures(url) {
        if let Ok(n) = caps[1].parse::<u32>() {
            return Some(n);
        }
    }

    None
}

/// 从文本中提取页码信息。
///
/// 支持格式：`Page 1 of 10`、`Page 1`、`共 100 条结果` 等。
/// 返回 (当前页, 总页数, 总条目数) 的三元组。
///
/// # 示例
///
/// ```
/// use crawlkit_parser::pagination::extract_page_info_from_text;
/// let (cur, total, items) = extract_page_info_from_text("Page 3 of 10");
/// assert_eq!(cur, Some(3));
/// assert_eq!(total, Some(10));
/// ```
pub fn extract_page_info_from_text(text: &str) -> (Option<u32>, Option<u32>, Option<u32>) {
    // 尝试 "Page X of Y" 格式
    if let Some(caps) = PAGE_TEXT.captures(text) {
        let current = caps[1].parse::<u32>().ok();
        let total = caps[2].parse::<u32>().ok();
        return (current, total, None);
    }

    // 尝试简单页码
    let current = SIMPLE_PAGE_TEXT.captures(text)
        .and_then(|caps| caps[1].parse::<u32>().ok());

    // 尝试总条目数
    let total_items = TOTAL_ITEMS_TEXT.captures(text)
        .and_then(|caps| caps[1].parse::<u32>().ok());

    (current, None, total_items)
}

// ============================================================================
// URL 解析与链接提取
// ============================================================================

/// 解析 URL，支持相对路径和协议相对 URL。
///
/// 使用已有的 `resolve_url` 逻辑，已重写以避免跨模块依赖。
pub fn resolve_url(href: &str, base_url: Option<&Url>) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    // 协议相对 URL
    if href.starts_with("//") {
        let scheme = base_url.map(|u| u.scheme()).unwrap_or("https");
        return Some(format!("{scheme}:{href}"));
    }

    // 已经是绝对 URL
    if href.starts_with("http://") || href.starts_with("https://") {
        return Url::parse(href).ok().map(|u| u.to_string());
    }

    // 相对 URL
    match base_url {
        Some(base) => base.join(href).ok().map(|u| u.to_string()),
        None => Some(href.to_string()),
    }
}

/// 从 `<link rel="next">`、`<link rel="prev">` 等标签中提取分页链接。
///
/// 检查 HTML `<head>` 中的 `<link>` 标签，提取 `rel="next"`、`rel="prev"`、
/// `rel="first"`、`rel="last"` 等分页信息。
pub fn extract_rel_links(document: &Html, base_url: Option<&Url>) -> Pagination {
    let selector = Selector::parse("link[rel][href]").expect("选择器 link[rel][href] 应合法");
    let mut pagination = Pagination::none();

    for element in document.select(&selector) {
        let rel = element.value().attr("rel").unwrap_or("").to_lowercase();
        let href = match element.value().attr("href") {
            Some(h) => resolve_url(h, base_url).unwrap_or_else(|| h.to_string()),
            None => continue,
        };

        match rel.as_str() {
            "next" => pagination.next_url = Some(href),
            "prev" | "previous" => pagination.prev_url = Some(href),
            "first" => pagination.first_url = Some(href),
            "last" => pagination.last_url = Some(href),
            _ => {}
        }
    }

    // 如果有 rel="next" 或 rel="prev"，标记为 NextPrev 类型
    if pagination.next_url.is_some() || pagination.prev_url.is_some() {
        pagination.pagination_type = PaginationType::NextPrev;
    }

    pagination
}

/// 从 DOM 中提取分页链接元素。
///
/// 使用常见的选择器匹配分页导航中的 `<a>` 标签，
/// 解析每个链接的 URL、页码和当前页状态。
pub fn extract_page_links(document: &Html, base_url: Option<&Url>) -> Vec<PageUrl> {
    // 常见分页导航选择器
    let selectors = [
        ".pagination a",
        ".pager a",
        ".page-nav a",
        ".page-navigation a",
        ".pages a",
        ".page-numbers",
        "nav.pagination a",
        "ul.pagination a",
        "div.pagination a",
        "[class*=\"pagination\"] a",
        "[class*=\"pager\"] a",
        "[class*=\"page-nav\"] a",
    ];

    let mut page_urls = Vec::new();
    let mut seen = HashSet::new();

    for selector_str in &selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };

        for element in document.select(&selector) {
            let href = match element.value().attr("href") {
                Some(h) => h.trim(),
                None => continue,
            };

            if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
                continue;
            }

            let resolved = resolve_url(href, base_url);
            let url_str = resolved.as_deref().unwrap_or(href).to_string();

            // 去重
            if !seen.insert(url_str.clone()) {
                continue;
            }

            // 判断是否为当前页
            let is_current = element.value().attr("class")
                .map(|c| c.contains("current") || c.contains("active"))
                .unwrap_or(false);

            // 尝试从链接文本提取页码
            let text: String = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let page_number =
                // 优先从 URL 提取
                extract_page_number_from_url(&url_str)
                // 其次从文本提取
                .or_else(|| {
                    SIMPLE_PAGE_TEXT.captures(&text)
                        .and_then(|caps| caps[1].parse::<u32>().ok())
                })
                // 最后尝试直接解析文本为数字
                .or_else(|| text.parse::<u32>().ok())
                // 默认为 0（表示无法确定）
                .unwrap_or(0);

            page_urls.push(PageUrl {
                url: url_str,
                page_number,
                is_current,
            });
        }
    }

    page_urls
}

// ============================================================================
// 分页模式检测
// ============================================================================

/// 检测页面是否使用无限滚动。
///
/// 通过检查 JavaScript 属性、类名、data 属性以及常见无限滚动库的标记来判断。
pub fn detect_infinite_scroll(document: &Html) -> bool {
    // 检查特定类名或属性
    let attr_selectors = [
        "[class*=\"infinite-scroll\"]",
        "[class*=\"infinite-scroll-container\"]",
        "[id*=\"infinite-scroll\"]",
        "[data-infinite-scroll]",
        "[data-infinite]",
        "[class*=\"endless-scroll\"]",
    ];

    for selector_str in &attr_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if document.select(&selector).next().is_some() {
                return true;
            }
        }
    }

    // 检查 `<script>` 内容
    let script_selector = Selector::parse("script").expect("script 选择器应合法");
    for element in document.select(&script_selector) {
        let content: String = element.text().collect();
        if INFINITE_SCROLL_PATTERN.is_match(&content) {
            return true;
        }
    }

    false
}

/// 检测页面是否使用「加载更多」按钮。
///
/// 通过检查常见 CSS 类名、按钮文本以及 data 属性来判断。
pub fn detect_load_more(document: &Html) -> bool {
    let selectors = [
        ".load-more",
        ".loadmore",
        ".show-more",
        ".view-more",
        "[class*=\"load-more\"]",
        "[class*=\"loadmore\"]",
        "[class*=\"show-more\"]",
        "[data-load-more]",
        "[data-loadmore]",
        "button.load-more",
        "a.load-more",
        "button.show-more",
        "a.show-more",
    ];

    for selector_str in &selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                let text: String = element.text().collect();
                if LOAD_MORE_TEXT.is_match(&text) {
                    return true;
                }
            }
        }
    }

    false
}

/// 综合判断分页类型。
///
/// 基于提取到的分页链接、DOM 属性、文本内容等综合判断分页模式。
pub fn determine_pagination_type(
    page_urls: &[PageUrl],
    has_infinite_scroll: bool,
    has_load_more: bool,
    document: &Html,
) -> PaginationType {
    if has_infinite_scroll {
        return PaginationType::InfiniteScroll;
    }

    if has_load_more {
        return PaginationType::LoadMore;
    }

    // 检查 URL 中是否包含游标参数
    let all_urls: Vec<&str> = page_urls.iter().map(|p| p.url.as_str()).collect();
    if all_urls.iter().any(|u| CURSOR_PATTERN.is_match(u)) {
        return PaginationType::Cursor;
    }

    // 检查 URL 中是否包含偏移量参数
    if all_urls.iter().any(|u| OFFSET_PATTERN.is_match(u)) {
        return PaginationType::Offset;
    }

    // 检查是否有超过两个带明确页码的链接
    let numbered_count = page_urls.iter().filter(|p| p.page_number > 0).count();
    if numbered_count >= 2 {
        return PaginationType::Numbered;
    }

    // 检查页面中是否有上一页/下一页文本标记
    let text_selector = Selector::parse("a, span, button").expect("选择器 a, span, button 应合法");
    let mut has_prev = false;
    let mut has_next = false;

    for element in document.select(&text_selector) {
        let text: String = element.text().collect();
        if PREV_TEXT.is_match(&text) {
            has_prev = true;
        }
        if NEXT_TEXT.is_match(&text) {
            has_next = true;
        }
        if has_prev && has_next {
            return PaginationType::NextPrev;
        }
    }

    PaginationType::None
}

// ============================================================================
// 主提取函数
// ============================================================================

/// 从 HTML 文档中提取完整的分页信息。
///
/// 综合使用 rel 链接、DOM 分页链接、文本内容检测、无限滚动/加载更多检测等手段，
/// 返回包含所有分页细节的 `Pagination` 结构。
///
/// # 参数
///
/// * `document` - 解析后的 HTML 文档
/// * `base_url` - 基准 URL，用于解析相对链接
/// * `html_content` - 原始 HTML 字符串（用于脚本内容检测）
///
/// # 示例
///
/// ```
/// use scraper::Html;
/// use crawlkit_parser::pagination::extract_pagination;
///
/// let html = r#"<html><body>
///     <div class="pagination">
///         <a href="?page=1" class="current">1</a>
///         <a href="?page=2">2</a>
///         <a href="?page=3">3</a>
///     </div>
/// </body></html>"#;
/// let document = Html::parse_document(html);
/// let pagination = extract_pagination(&document, None, html);
/// assert!(pagination.has_pagination());
/// assert_eq!(pagination.page_urls.len(), 3);
/// ```
pub fn extract_pagination(
    document: &Html,
    base_url: Option<&Url>,
    _html_content: &str,
) -> Pagination {
    let mut pagination = Pagination::none();

    // 1. 从 <link rel> 标签提取分页链接
    let rel_pagination = extract_rel_links(document, base_url);
    pagination.next_url = rel_pagination.next_url;
    pagination.prev_url = rel_pagination.prev_url;
    pagination.first_url = rel_pagination.first_url;
    pagination.last_url = rel_pagination.last_url;

    // 2. 从 DOM 提取分页链接列表
    let page_urls = extract_page_links(document, base_url);
    pagination.page_urls = page_urls;

    // 3. 尝试从分页链接中推断当前页码
    let current_from_urls = pagination.page_urls.iter()
        .find(|p| p.is_current)
        .map(|p| p.page_number);

    let current_from_next = pagination.next_url.as_deref()
        .and_then(extract_page_number_from_url)
        .map(|n| n.saturating_sub(1));

    // 尝试从 rel prev 推断当前页
    let current_from_prev = pagination.prev_url.as_deref()
        .and_then(extract_page_number_from_url)
        .map(|n| n.saturating_add(1));

    pagination.current_page = current_from_urls
        .or(current_from_prev)
        .or(current_from_next)
        .unwrap_or(1);

    // 4. 检测无限滚动
    pagination.has_infinite_scroll = detect_infinite_scroll(document);

    // 5. 检测加载更多
    pagination.has_load_more = detect_load_more(document);

    // 6. 综合判断分页类型
    pagination.pagination_type = determine_pagination_type(
        &pagination.page_urls,
        pagination.has_infinite_scroll,
        pagination.has_load_more,
        document,
    );

    // 7. 尝试从文本提取总页数等信息
    let text_selector = Selector::parse("body").expect("body 选择器应合法");
    if let Some(body) = document.select(&text_selector).next() {
        let body_text: String = body.text().collect();
        let (cur, total, items) = extract_page_info_from_text(&body_text);
        if pagination.current_page == 1 && cur.is_some() {
            pagination.current_page = cur.unwrap_or(1);
        }
        pagination.total_pages = pagination.total_pages.or(total);
        pagination.total_items = pagination.total_items.or(items);
    }

    // 8. 从 URL 查询参数推断总页数（如果有 total 参数）
    if let Some(ref base) = base_url {
        if let Some(total_str) = base.query_pairs()
            .find(|(k, _)| k == "total" || k == "pages")
            .map(|(_, v)| v.to_string())
        {
            if let Ok(total) = total_str.parse::<u32>() {
                pagination.total_pages = Some(total);
            }
        }
    }

    pagination
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速判断 HTML 文档中是否包含分页。
///
/// 检查 rel 链接、分页 DOM 元素、无限滚动标记、加载更多按钮等。
///
/// # 示例
///
/// ```
/// use scraper::Html;
/// use crawlkit_parser::pagination::has_pagination;
///
/// let html = r#"<html><body><div class="pagination"><a href="?page=2">2</a></div></body></html>"#;
/// let document = Html::parse_document(html);
/// assert!(has_pagination(&document, html));
/// ```
pub fn has_pagination(document: &Html, _html_content: &str) -> bool {
    // 检查 rel 链接
    let rel_selector = Selector::parse("link[rel=\"next\"], link[rel=\"prev\"]")
        .expect("选择器应合法");
    if document.select(&rel_selector).next().is_some() {
        return true;
    }

    // 检查分页 DOM 元素
    let pagination_classes = [
        ".pagination",
        ".pager",
        ".page-nav",
        ".page-numbers",
        "[class*=\"pagination\"]",
    ];
    let any_pagination = pagination_classes.iter().any(|cls| {
        Selector::parse(cls)
            .ok()
            .map(|sel| document.select(&sel).next().is_some())
            .unwrap_or(false)
    });
    if any_pagination {
        return true;
    }

    // 检查无限滚动
    if detect_infinite_scroll(document) {
        return true;
    }

    // 检查加载更多
    if detect_load_more(document) {
        return true;
    }

    // 检查常见分页文本
    let body_selector = Selector::parse("body").expect("body 选择器应合法");
    if let Some(body) = document.select(&body_selector).next() {
        let text: String = body.text().collect();
        if PREV_TEXT.is_match(&text) || NEXT_TEXT.is_match(&text) || PAGE_TEXT.is_match(&text) {
            return true;
        }
    }

    false
}

/// 获取下一页的 URL。
///
/// 优先从 `<link rel="next">` 获取，其次从分页 DOM 中推断。
///
/// # 示例
///
/// ```
/// use scraper::Html;
/// use crawlkit_parser::pagination::get_next_page;
///
/// let html = r#"<html><head><link rel="next" href="https://example.com?page=2"></head></html>"#;
/// let document = Html::parse_document(html);
/// assert_eq!(get_next_page(&document, None), Some("https://example.com/?page=2".to_string()));
/// ```
pub fn get_next_page(document: &Html, base_url: Option<&Url>) -> Option<String> {
    // 优先从 rel="next" 获取
    if let Some(url) = extract_rel_link(document, "next", base_url) {
        return Some(url);
    }

    // 从分页 DOM 中找带有「下一页」文本的链接
    let selectors = [
        "a.next",
        "a.next-page",
        "a[rel=\"next\"]",
        ".pagination a:last-child",
        ".pager a:last-child",
    ];
    for selector_str in &selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };
        for element in document.select(&selector) {
            let text: String = element.text().collect();
            if NEXT_TEXT.is_match(&text) {
                if let Some(href) = element.value().attr("href") {
                    return resolve_url(href, base_url);
                }
            }
        }
    }

    // 从分页链接中找出比当前页大 1 的链接
    let page_urls = extract_page_links(document, base_url);
    let current = page_urls.iter().find(|p| p.is_current).map(|p| p.page_number);
    if let Some(cur) = current {
        if let Some(next) = page_urls.iter().find(|p| p.page_number == cur + 1) {
            return Some(next.url.clone());
        }
    }

    None
}

/// 获取上一页的 URL。
///
/// 优先从 `<link rel="prev">` 获取，其次从分页 DOM 中推断。
///
/// # 示例
///
/// ```
/// use scraper::Html;
/// use crawlkit_parser::pagination::get_prev_page;
///
/// let html = r#"<html><head><link rel="prev" href="https://example.com?page=1"></head></html>"#;
/// let document = Html::parse_document(html);
/// assert_eq!(get_prev_page(&document, None), Some("https://example.com/?page=1".to_string()));
/// ```
pub fn get_prev_page(document: &Html, base_url: Option<&Url>) -> Option<String> {
    // 优先从 rel="prev" 获取
    if let Some(url) = extract_rel_link(document, "prev", base_url) {
        return Some(url);
    }

    // 从分页 DOM 中找带有「上一页」文本的链接
    let selectors = [
        "a.prev",
        "a.previous",
        "a.prev-page",
        "a[rel=\"prev\"]",
        ".pagination a:first-child",
        ".pager a:first-child",
    ];
    for selector_str in &selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };
        for element in document.select(&selector) {
            let text: String = element.text().collect();
            if PREV_TEXT.is_match(&text) {
                if let Some(href) = element.value().attr("href") {
                    return resolve_url(href, base_url);
                }
            }
        }
    }

    // 从分页链接中找出比当前页小 1 的链接
    let page_urls = extract_page_links(document, base_url);
    let current = page_urls.iter().find(|p| p.is_current).map(|p| p.page_number);
    if let Some(cur) = current {
        if cur > 1 {
            if let Some(prev) = page_urls.iter().find(|p| p.page_number == cur - 1) {
                return Some(prev.url.clone());
            }
        }
    }

    None
}

/// 根据基准 URL 和页码生成分页 URL。
///
/// 检测原始 URL 中的页码模式（查询参数或路径），替换为新的页码后返回。
///
/// # 示例
///
/// ```
/// use url::Url;
/// use crawlkit_parser::pagination::generate_page_url;
///
/// let base = Url::parse("https://example.com?page=1").unwrap();
/// assert_eq!(generate_page_url(&base, 3), Some("https://example.com/?page=3".to_string()));
/// ```
pub fn generate_page_url(base_url: &Url, page_num: u32) -> Option<String> {
    // 如果 URL 已包含 page 参数，替换它
    if PAGE_IN_QUERY.is_match(base_url.as_str()) {
        let result = PAGE_IN_QUERY.replace(base_url.as_str(), |caps: &regex::Captures| {
            // 保留前缀（? 或 &）和参数名，只替换值
            let prefix = &caps[0][..caps[0].len() - caps[1].len()];
            format!("{}{}", prefix, page_num)
        });
        return Some(result.to_string());
    }

    // 如果 URL 路径中包含 /page/N，替换之
    if PAGE_IN_PATH.is_match(base_url.as_str()) {
        let result = PAGE_IN_PATH.replace(base_url.as_str(), |caps: &regex::Captures| {
            let prefix = &caps[0][..caps[0].len() - caps[1].len()];
            format!("{}{}", prefix, page_num)
        });
        return Some(result.to_string());
    }

    // 否则拼接 page 查询参数
    let mut url = base_url.clone();
    url.query_pairs_mut().append_pair("page", &page_num.to_string());
    Some(url.to_string())
}

// ============================================================================
// 内部辅助函数
// ============================================================================

/// 从 `<link rel="...">` 标签中提取指定 rel 值的 href 属性。
fn extract_rel_link(document: &Html, rel_value: &str, base_url: Option<&Url>) -> Option<String> {
    let selector_str = format!("link[rel=\"{}\"][href]", rel_value);
    let selector = Selector::parse(&selector_str).ok()?;
    let element = document.select(&selector).next()?;
    let href = element.value().attr("href")?;
    resolve_url(href, base_url)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 辅助：使用 None base_url 提取分页
    fn pagination_from_html(html: &str) -> Pagination {
        let document = Html::parse_document(html);
        extract_pagination(&document, None, html)
    }

    // ========================================================================
    // 基础测试：无分页页面
    // ========================================================================

    #[test]
    fn test_no_pagination() {
        let html = "<html><body><p>Hello, world!</p></body></html>";
        let pag = pagination_from_html(html);
        assert_eq!(pag.pagination_type, PaginationType::None);
        assert!(!pag.has_pagination());
        assert!(pag.page_urls.is_empty());
        assert!(pag.next_url.is_none());
        assert!(pag.prev_url.is_none());
        assert_eq!(pag.current_page, 1);
    }

    // ========================================================================
    // 测试：从 URL 提取页码
    // ========================================================================

    #[test]
    fn test_extract_page_number_from_query() {
        assert_eq!(extract_page_number_from_url("https://example.com?page=3"), Some(3));
        assert_eq!(extract_page_number_from_url("https://example.com?p=5"), Some(5));
        assert_eq!(extract_page_number_from_url("https://example.com?pg=2"), Some(2));
    }

    #[test]
    fn test_extract_page_number_from_path() {
        assert_eq!(extract_page_number_from_url("https://example.com/page/3"), Some(3));
        assert_eq!(extract_page_number_from_url("https://example.com/page/10/"), Some(10));
    }

    #[test]
    fn test_extract_page_number_from_numeric_suffix() {
        assert_eq!(extract_page_number_from_url("https://example.com/articles/42/"), Some(42));
    }

    #[test]
    fn test_extract_page_number_no_match() {
        assert_eq!(extract_page_number_from_url("https://example.com/about"), None);
        assert_eq!(extract_page_number_from_url("https://example.com"), None);
    }

    #[test]
    fn test_extract_page_number_case_insensitive() {
        assert_eq!(extract_page_number_from_url("https://example.com?Page=7"), Some(7));
        assert_eq!(extract_page_number_from_url("https://example.com/PAGE/2"), Some(2));
    }

    // ========================================================================
    // 测试：从文本提取页码信息
    // ========================================================================

    #[test]
    fn test_extract_page_info_full() {
        let (cur, total, items) = extract_page_info_from_text("Page 3 of 10");
        assert_eq!(cur, Some(3));
        assert_eq!(total, Some(10));
        assert_eq!(items, None);
    }

    #[test]
    fn test_extract_page_info_simple() {
        let (cur, total, items) = extract_page_info_from_text("Page 5");
        assert_eq!(cur, Some(5));
        assert_eq!(total, None);
        assert_eq!(items, None);
    }

    #[test]
    fn test_extract_page_info_total_items() {
        let (cur, total, items) = extract_page_info_from_text("共 200 条结果");
        assert_eq!(cur, None);
        assert_eq!(total, None);
        assert_eq!(items, Some(200));
    }

    #[test]
    fn test_extract_page_info_no_match() {
        let (cur, total, items) = extract_page_info_from_text("Hello World");
        assert_eq!(cur, None);
        assert_eq!(total, None);
        assert_eq!(items, None);
    }

    // ========================================================================
    // 测试：解析 URL
    // ========================================================================

    #[test]
    fn test_resolve_url_absolute() {
        let base = Url::parse("https://example.com").ok();
        let result = resolve_url("https://other.com/path", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://other.com/path"));
    }

    #[test]
    fn test_resolve_url_relative() {
        let base = Url::parse("https://example.com/base/").ok();
        let result = resolve_url("../page", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn test_resolve_url_protocol_relative() {
        let base = Url::parse("https://example.com").ok();
        let result = resolve_url("//other.com/path", base.as_ref());
        assert_eq!(result.as_deref(), Some("https://other.com/path"));
    }

    #[test]
    fn test_resolve_url_empty() {
        assert!(resolve_url("", None).is_none());
        assert!(resolve_url("  ", None).is_none());
    }

    // ========================================================================
    // 测试：提取 rel 链接
    // ========================================================================

    #[test]
    fn test_extract_rel_links_next_prev() {
        let html = r#"<html><head>
            <link rel="next" href="https://example.com?page=2">
            <link rel="prev" href="https://example.com?page=1">
        </head></html>"#;
        let document = Html::parse_document(html);
        let pag = extract_rel_links(&document, None);
        assert_eq!(pag.next_url.as_deref(), Some("https://example.com/?page=2"));
        assert_eq!(pag.prev_url.as_deref(), Some("https://example.com/?page=1"));
        assert_eq!(pag.pagination_type, PaginationType::NextPrev);
    }

    #[test]
    fn test_extract_rel_links_first_last() {
        let html = r#"<html><head>
            <link rel="first" href="https://example.com">
            <link rel="last" href="https://example.com?page=50">
        </head></html>"#;
        let document = Html::parse_document(html);
        let pag = extract_rel_links(&document, None);
        assert_eq!(pag.first_url.as_deref(), Some("https://example.com/"));
        assert_eq!(pag.last_url.as_deref(), Some("https://example.com/?page=50"));
    }

    // ========================================================================
    // 测试：提取分页链接
    // ========================================================================

    #[test]
    fn test_extract_page_links_numbered() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="?page=1" class="current">1</a>
                <a href="?page=2">2</a>
                <a href="?page=3">3</a>
            </div>
        </body></html>"#;
        let document = Html::parse_document(html);
        let links = extract_page_links(&document, None);
        assert_eq!(links.len(), 3);
        assert!(links[0].is_current);
        assert_eq!(links[0].page_number, 1);
        assert!(!links[1].is_current);
        assert_eq!(links[1].page_number, 2);
        assert_eq!(links[2].page_number, 3);
    }

    #[test]
    fn test_extract_page_links_with_text_numbers() {
        let html = r#"<html><body>
            <ul class="pagination">
                <li><a href="/page/1">1</a></li>
                <li><a href="/page/2" class="active">2</a></li>
                <li><a href="/page/3">3</a></li>
            </ul>
        </body></html>"#;
        let document = Html::parse_document(html);
        let links = extract_page_links(&document, None);
        // 链接文本可解析为数字
        assert_eq!(links.len(), 3);
        // /page/1 => 页码 1; /page/2 => 页码 2
        assert_eq!(links[0].page_number, 1);
        assert!(!links[0].is_current);
        assert_eq!(links[1].page_number, 2);
        assert!(links[1].is_current);
    }

    #[test]
    fn test_extract_page_links_deduplicates() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="?page=1">1</a>
                <a href="?page=2">2</a>
            </div>
            <nav class="pagination">
                <a href="?page=1">1</a>
                <a href="?page=2">2</a>
            </nav>
        </body></html>"#;
        let document = Html::parse_document(html);
        let links = extract_page_links(&document, None);
        assert_eq!(links.len(), 2); // 去重后应为 2
    }

    // ========================================================================
    // 测试：无限滚动检测
    // ========================================================================

    #[test]
    fn test_detect_infinite_scroll_by_class() {
        let html = r#"<html><body><div class="infinite-scroll"></div></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_infinite_scroll(&document));
    }

    #[test]
    fn test_detect_infinite_scroll_by_data_attr() {
        let html = r#"<html><body><div data-infinite-scroll="true"></div></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_infinite_scroll(&document));
    }

    #[test]
    fn test_detect_infinite_scroll_by_script() {
        let html = r#"<html><body><script>var infiniteScroll = true;</script></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_infinite_scroll(&document));
    }

    #[test]
    fn test_detect_infinite_scroll_none() {
        let html = r#"<html><body><p>No scroll here</p></body></html>"#;
        let document = Html::parse_document(html);
        assert!(!detect_infinite_scroll(&document));
    }

    // ========================================================================
    // 测试：加载更多检测
    // ========================================================================

    #[test]
    fn test_detect_load_more_by_class() {
        let html = r#"<html><body><button class="load-more">加载更多</button></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_load_more(&document));
    }

    #[test]
    fn test_detect_load_more_by_text() {
        let html = r#"<html><body><a class="show-more">查看更多</a></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_load_more(&document));
    }

    #[test]
    fn test_detect_load_more_english() {
        let html = r#"<html><body><button class="load-more">Load More</button></body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_load_more(&document));
    }

    #[test]
    fn test_detect_load_more_none() {
        let html = r#"<html><body><button>提交</button></body></html>"#;
        let document = Html::parse_document(html);
        assert!(!detect_load_more(&document));
    }

    // ========================================================================
    // 测试：分页类型判断
    // ========================================================================

    #[test]
    fn test_determine_pagination_type_numbered() {
        let urls = vec![
            PageUrl::new("?page=1", 1, true),
            PageUrl::new("?page=2", 2, false),
            PageUrl::new("?page=3", 3, false),
        ];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, false, false, &document);
        assert_eq!(ptype, PaginationType::Numbered);
    }

    #[test]
    fn test_determine_pagination_type_infinite_scroll() {
        let urls = vec![];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, true, false, &document);
        assert_eq!(ptype, PaginationType::InfiniteScroll);
    }

    #[test]
    fn test_determine_pagination_type_load_more() {
        let urls = vec![];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, false, true, &document);
        assert_eq!(ptype, PaginationType::LoadMore);
    }

    #[test]
    fn test_determine_pagination_type_next_prev_from_text() {
        let urls = vec![];
        let html = r#"<html><body><a>上一页</a><a>下一页</a></body></html>"#;
        let document = Html::parse_document(html);
        let ptype = determine_pagination_type(&urls, false, false, &document);
        assert_eq!(ptype, PaginationType::NextPrev);
    }

    #[test]
    fn test_determine_pagination_type_cursor() {
        let urls = vec![
            PageUrl::new("https://example.com?cursor=abc", 0, false),
        ];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, false, false, &document);
        assert_eq!(ptype, PaginationType::Cursor);
    }

    #[test]
    fn test_determine_pagination_type_offset() {
        let urls = vec![
            PageUrl::new("https://example.com?offset=10", 0, false),
        ];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, false, false, &document);
        assert_eq!(ptype, PaginationType::Offset);
    }

    // ========================================================================
    // 测试：主提取函数
    // ========================================================================

    #[test]
    fn test_extract_pagination_numbered() {
        let html = r#"<html><head>
            <link rel="next" href="?page=2">
        </head><body>
            <div class="pagination">
                <a href="?page=1" class="current">1</a>
                <a href="?page=2">2</a>
                <a href="?page=3">3</a>
            </div>
        </body></html>"#;
        let pag = pagination_from_html(html);
        assert_eq!(pag.pagination_type, PaginationType::Numbered);
        assert!(pag.has_pagination());
        assert_eq!(pag.page_urls.len(), 3);
        assert_eq!(pag.current_page, 1);
        assert_eq!(pag.next_url.as_deref(), Some("?page=2"));
    }

    #[test]
    fn test_extract_pagination_with_base_url() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="/page/2">2</a>
                <a href="/page/3">3</a>
            </div>
        </body></html>"#;
        let base = Url::parse("https://example.com/page/1").ok();
        let document = Html::parse_document(html);
        let pag = extract_pagination(&document, base.as_ref(), html);
        assert!(pag.has_pagination());
        assert_eq!(pag.page_urls.len(), 2);
        assert_eq!(pag.page_urls[0].url, "https://example.com/page/2");
        assert_eq!(pag.page_urls[1].url, "https://example.com/page/3");
    }

    #[test]
    fn test_extract_pagination_infinite_scroll() {
        let html = r#"<html><body>
            <div class="infinite-scroll" data-infinite-scroll="true"></div>
            <script>var infiniteScroll = true;</script>
        </body></html>"#;
        let pag = pagination_from_html(html);
        assert_eq!(pag.pagination_type, PaginationType::InfiniteScroll);
        assert!(pag.has_infinite_scroll);
        assert!(pag.has_pagination());
    }

    #[test]
    fn test_extract_pagination_load_more() {
        let html = r#"<html><body>
            <button class="load-more">加载更多</button>
        </body></html>"#;
        let pag = pagination_from_html(html);
        assert_eq!(pag.pagination_type, PaginationType::LoadMore);
        assert!(pag.has_load_more);
        assert!(pag.has_pagination());
    }

    // ========================================================================
    // 测试：便捷函数
    // ========================================================================

    #[test]
    fn test_has_pagination_pagination_class() {
        let html = r#"<html><body><div class="pagination"></div></body></html>"#;
        let document = Html::parse_document(html);
        assert!(has_pagination(&document, html));
    }

    #[test]
    fn test_has_pagination_rel_links() {
        let html = r#"<html><head><link rel="next" href="?page=2"></head></html>"#;
        let document = Html::parse_document(html);
        assert!(has_pagination(&document, html));
    }

    #[test]
    fn test_has_pagination_no_pagination() {
        let html = "<html><body><p>No pagination</p></body></html>";
        let document = Html::parse_document(html);
        assert!(!has_pagination(&document, html));
    }

    #[test]
    fn test_get_next_page_from_rel() {
        let html = r#"<html><head><link rel="next" href="https://example.com?page=2"></head></html>"#;
        let document = Html::parse_document(html);
        assert_eq!(
            get_next_page(&document, None),
            Some("https://example.com/?page=2".to_string())
        );
    }

    #[test]
    fn test_get_next_page_from_dom() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="?page=1" class="current">1</a>
                <a href="?page=2">2</a>
                <a href="?page=3">3</a>
            </div>
        </body></html>"#;
        let document = Html::parse_document(html);
        // 会找到 page=2（比当前页大 1）
        let next = get_next_page(&document, None);
        assert!(next.is_some());
    }

    #[test]
    fn test_get_next_page_not_found() {
        let html = "<html><body><p>No pagination</p></body></html>";
        let document = Html::parse_document(html);
        assert!(get_next_page(&document, None).is_none());
    }

    #[test]
    fn test_get_prev_page_from_rel() {
        let html = r#"<html><head><link rel="prev" href="https://example.com?page=1"></head></html>"#;
        let document = Html::parse_document(html);
        assert_eq!(
            get_prev_page(&document, None),
            Some("https://example.com/?page=1".to_string())
        );
    }

    #[test]
    fn test_generate_page_url_replace_query() {
        let base = Url::parse("https://example.com?page=1").unwrap();
        assert_eq!(
            generate_page_url(&base, 3),
            Some("https://example.com/?page=3".to_string())
        );
    }

    #[test]
    fn test_generate_page_url_replace_path() {
        let base = Url::parse("https://example.com/page/1").unwrap();
        let result = generate_page_url(&base, 5);
        assert!(result.is_some());
        assert!(result.unwrap().contains("page/5"));
    }

    #[test]
    fn test_generate_page_url_append_query() {
        let base = Url::parse("https://example.com/search?q=rust").unwrap();
        let result = generate_page_url(&base, 2).unwrap();
        assert!(result.contains("page=2"));
    }

    // ========================================================================
    // 测试：Pagination 对象方法
    // ========================================================================

    #[test]
    fn test_pagination_has_pagination_false() {
        let pag = Pagination::none();
        assert!(!pag.has_pagination());
    }

    #[test]
    fn test_pagination_has_pagination_with_page_urls() {
        let mut pag = Pagination::none();
        pag.page_urls.push(PageUrl::new("?page=2", 2, false));
        pag.page_urls.push(PageUrl::new("?page=3", 3, false));
        assert!(pag.has_pagination());
    }

    #[test]
    fn test_pagination_numbered_pages() {
        let mut pag = Pagination::none();
        pag.next_url = Some("?page=4".to_string());
        pag.prev_url = Some("?page=2".to_string());
        pag.page_urls = vec![
            PageUrl::new("?page=2", 2, false),
            PageUrl::new("?page=3", 3, true),
            PageUrl::new("?page=4", 4, false),
        ];
        let numbered = pag.numbered_pages();
        // page_urls 共 3 条，其中 next_url 和 prev_url 各匹配一条
        assert_eq!(numbered.len(), 1);
        assert_eq!(numbered[0].page_number, 3);
    }

    #[test]
    fn test_pagination_none_is_default() {
        let pag = Pagination::default();
        assert_eq!(pag.pagination_type, PaginationType::None);
        assert_eq!(pag.current_page, 1);
        assert!(pag.page_urls.is_empty());
    }

    // ========================================================================
    // 测试：边界情况
    // ========================================================================

    #[test]
    fn test_extract_page_number_from_url_empty() {
        assert_eq!(extract_page_number_from_url(""), None);
    }

    #[test]
    fn test_extract_page_links_empty_html() {
        let document = Html::parse_document("<html></html>");
        let links = extract_page_links(&document, None);
        assert!(links.is_empty());
    }

    #[test]
    fn test_determine_pagination_type_empty() {
        let urls = vec![];
        let document = Html::parse_document("<html></html>");
        let ptype = determine_pagination_type(&urls, false, false, &document);
        assert_eq!(ptype, PaginationType::None);
    }

    #[test]
    fn test_resolve_url_http_scheme() {
        let base = Url::parse("http://example.com").ok();
        let result = resolve_url("//cdn.example.com/file.js", base.as_ref());
        assert_eq!(result.as_deref(), Some("http://cdn.example.com/file.js"));
    }

    #[test]
    fn test_extract_page_number_ampersand_param() {
        assert_eq!(
            extract_page_number_from_url("https://example.com?cat=news&page=4"),
            Some(4)
        );
    }

    #[test]
    fn test_detect_load_more_multiple_classes() {
        let html = r#"<html><body>
            <div class="show-more">Load More</div>
            <div class="load-more">加载更多</div>
        </body></html>"#;
        let document = Html::parse_document(html);
        assert!(detect_load_more(&document));
    }

    #[test]
    fn test_extract_rel_links_with_base_resolution() {
        let html = r#"<html><head>
            <link rel="next" href="/page/2">
        </head></html>"#;
        let base = Url::parse("https://example.com/news/").ok();
        let document = Html::parse_document(html);
        let pag = extract_rel_links(&document, base.as_ref());
        assert_eq!(
            pag.next_url.as_deref(),
            Some("https://example.com/page/2")
        );
    }

    #[test]
    fn test_extract_page_links_text_number_fallback() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="/news?page=abc">3</a>
            </div>
        </body></html>"#;
        let document = Html::parse_document(html);
        let links = extract_page_links(&document, None);
        // URL 中的 page=abc 无法解析，但链接文本 "3" 可解析为数字
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].page_number, 3);
    }

    // ========================================================================
    // 测试：PageUrl 构造
    // ========================================================================

    #[test]
    fn test_page_url_new() {
        let p = PageUrl::new("https://example.com?page=5", 5, true);
        assert_eq!(p.url, "https://example.com?page=5");
        assert_eq!(p.page_number, 5);
        assert!(p.is_current);
    }

    // ========================================================================
    // 测试：获取上一页/下一页带中文文本
    // ========================================================================

    #[test]
    fn test_get_next_page_chinese_text() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="?page=2" class="next">下一页</a>
            </div>
        </body></html>"#;
        let document = Html::parse_document(html);
        let next = get_next_page(&document, None);
        assert!(next.is_some());
    }

    #[test]
    fn test_get_prev_page_chinese_text() {
        let html = r#"<html><body>
            <div class="pagination">
                <a href="?page=1" class="prev">上一页</a>
            </div>
        </body></html>"#;
        let document = Html::parse_document(html);
        let prev = get_prev_page(&document, None);
        assert!(prev.is_some());
    }

    // ========================================================================
    // 测试：PaginationType 序列化
    // ========================================================================

    #[test]
    fn test_pagination_type_serialization() {
        assert_eq!(
            serde_json::to_string(&PaginationType::InfiniteScroll).unwrap(),
            "\"infinite_scroll\""
        );
        assert_eq!(
            serde_json::to_string(&PaginationType::LoadMore).unwrap(),
            "\"load_more\""
        );
        assert_eq!(
            serde_json::to_string(&PaginationType::Numbered).unwrap(),
            "\"numbered\""
        );
    }
}
