//! Collector 核心调度器
//!
//! 负责管理回调链、调度 HTTP 请求、跟踪已访问 URL 防止重复。
//! 设计参考 Go colly 的 Collector，采用 Builder + 回调模式。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use tokio::sync::Semaphore;
use tracing::{debug, instrument, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::request::Request;
use crawlkit_core::response::Response;
#[cfg(feature = "fetcher-reqwest")]
use crawlkit_fetcher_reqwest::ReqwestClient;
#[cfg(feature = "fetcher-wreq")]
use crawlkit_fetcher_wreq::WreqClient;
use crawlkit_parser::html::{extract_absolute_links, extract_article, sanitize_for_xpath, Article, LinkSelectorType};
use crawlkit_parser::scraper::{Html, Selector};
use crawlkit_parser::skyscraper::html as xpath_html;
use crawlkit_parser::skyscraper::xpath::{self as skyscraper_xpath, XpathItemTree};

/// follow_links 默认最大递归深度
const DEFAULT_MAX_DEPTH: usize = 10;

/// 默认并发上限（当 max_concurrency 为 0 时使用）
const DEFAULT_MAX_CONCURRENCY: usize = 16;

/// 回调函数类型别名（Arc 使得 Collector 可 Clone、回调可共享）
type RequestCallback = Arc<dyn Fn(&mut Request) + Send + Sync>;
type ResponseCallback = Arc<dyn Fn(&Response) + Send + Sync>;
type HtmlCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;
type ErrorCallback = Arc<dyn Fn(&dyn std::error::Error) + Send + Sync>;
type ResponseHeadersCallback = Arc<dyn Fn(&Response) + Send + Sync>;
type ScrapedCallback = Arc<dyn Fn(&Response) + Send + Sync>;

/// HTML 元素回调（CSS 选择器匹配）
type HtmlElementCallback = Arc<dyn Fn(&Element) + Send + Sync>;

/// XML 元素回调（XPath 匹配）
type XmlElementCallback = Arc<dyn Fn(&Element) + Send + Sync>;

/// 域名限速规则
#[derive(Debug, Clone)]
pub struct LimitRule {
    /// 域名通配模式（如 `"*example.com"`, `"*httpbin.*"`）
    pub domain_glob: String,
    /// 最大并发请求数
    pub parallelism: usize,
    /// 请求间隔
    pub delay: Duration,
    /// 额外随机延迟（0 表示不启用）
    pub random_delay: Duration,
}

impl LimitRule {
    /// 创建新规则
    pub fn new(domain_glob: &str) -> Self {
        Self {
            domain_glob: domain_glob.to_string(),
            parallelism: 1,
            delay: Duration::ZERO,
            random_delay: Duration::ZERO,
        }
    }

    /// 检查域名是否匹配此规则
    pub fn matches(&self, domain: &str) -> bool {
        let pattern = self.domain_glob.replace('*', ".*");
        Regex::new(&pattern)
            .is_ok_and(|re| re.is_match(domain))
    }
}

impl Default for LimitRule {
    fn default() -> Self {
        Self {
            domain_glob: "*".to_string(),
            parallelism: 1,
            delay: Duration::ZERO,
            random_delay: Duration::ZERO,
        }
    }
}

/// HTML 元素包装器
///
/// 在 `on_html_element` / `on_xml_element` 回调中使用，
/// 提供对匹配元素的文本、属性、HTML 内容的访问。
/// 设计参考 go-colly 的 `HTMLElement`。
pub struct Element<'a> {
    /// 标签名（如 `"a"`, `"div"`）
    pub name: String,
    /// 当前页面 URL（重定向后的最终 URL）
    pub url: &'a str,
    /// 元素的纯文本内容
    text: String,
    /// 元素属性
    attrs: HashMap<String, String>,
    /// 元素原始 HTML（含子元素）
    html: String,
    /// 在当前匹配结果中的位置索引
    pub index: usize,
}

impl<'a> Element<'a> {
    fn new(
        name: String,
        url: &'a str,
        text: String,
        attrs: HashMap<String, String>,
        html: String,
        index: usize,
    ) -> Self {
        Self {
            name,
            url,
            text,
            attrs,
            html,
            index,
        }
    }

    /// 获取元素的纯文本内容
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 获取元素的指定属性值
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    /// 获取元素的原始 HTML（含子元素）
    pub fn html(&self) -> &str {
        &self.html
    }

    /// 将属性值解析为绝对 URL
    ///
    /// 根据页面 URL 解析相对路径：
    /// - `/docs/` → `https://example.com/docs/`
    /// - `https://full.com/path` → 不变
    pub fn absolute_url(&self, attr_name: &str) -> Option<String> {
        let value = self.attr(attr_name)?;
        let page_url = url::Url::parse(self.url).ok()?;
        let resolved = page_url.join(value).ok()?;
        Some(resolved.to_string())
    }

    /// 获取匹配指定 CSS 选择器的第一个子元素的文本
    ///
    /// 类似 go-colly 的 `Element.ChildText(selector)`
    pub fn child_text(&self, selector: &str) -> String {
        let doc = Html::parse_document(&self.html);
        match Selector::parse(selector) {
            Ok(sel) => doc
                .select(&sel)
                .next()
                .map(|el| {
                    el.text()
                        .collect::<Vec<_>>()
                        .join("")
                        .trim()
                        .to_string()
                })
                .unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    /// 获取匹配指定 CSS 选择器的第一个子元素的属性值
    ///
    /// 类似 go-colly 的 `Element.ChildAttr(selector, attrName)`
    pub fn child_attr(&self, selector: &str, attr_name: &str) -> Option<String> {
        let doc = Html::parse_document(&self.html);
        Selector::parse(selector)
            .ok()
            .and_then(|sel| {
                doc.select(&sel)
                    .next()
                    .and_then(|el| el.value().attr(attr_name).map(ToString::to_string))
            })
    }

    /// 获取匹配指定 CSS 选择器的所有子元素的文本
    ///
    /// 类似 go-colly 的 `Element.ChildTexts(selector)`
    pub fn child_texts(&self, selector: &str) -> Vec<String> {
        let doc = Html::parse_document(&self.html);
        match Selector::parse(selector) {
            Ok(sel) => doc
                .select(&sel)
                .filter_map(|el| {
                    let text = el
                        .text()
                        .collect::<Vec<_>>()
                        .join("")
                        .trim()
                        .to_string();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 获取匹配指定 CSS 选择器的所有子元素的指定属性值
    ///
    /// 类似 go-colly 的 `Element.ChildAttrs(selector, attrName)`
    pub fn child_attrs(&self, selector: &str, attr_name: &str) -> Vec<String> {
        let doc = Html::parse_document(&self.html);
        match Selector::parse(selector) {
            Ok(sel) => doc
                .select(&sel)
                .filter_map(|el| {
                    el.value()
                        .attr(attr_name)
                        .map(ToString::to_string)
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// 获取所有属性的迭代器
    pub fn attrs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attrs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// 爬虫收集器 —— 框架核心调度器
///
/// # 使用示例
/// ```rust,no_run
/// use crawlkit::Collector;
///
/// #[tokio::main]
/// async fn main() {
///     let mut c = Collector::new();
///     c.on_request(|req| println!("请求: {}", req.url));
///     c.visit("https://example.com").await.unwrap();
/// }
/// ```
pub struct Collector {
    /// HTTP 客户端（可替换）
    http_client: Arc<dyn HttpClient>,

    /// 请求前回调
    on_request: Option<RequestCallback>,

    /// 收到响应头后回调（早于 on_response）
    on_response_headers: Option<ResponseHeadersCallback>,

    /// 收到响应后回调
    on_response: Option<ResponseCallback>,

    /// HTML 解析后回调
    on_html: Option<HtmlCallback>,

    /// 错误回调
    on_error: Option<ErrorCallback>,

    /// CSS 选择器 → HTML 元素回调列表
    on_html_elements: Vec<(String, HtmlElementCallback)>,

    /// XPath → XML 元素回调列表
    on_xml_elements: Vec<(String, XmlElementCallback)>,

    /// 抓取完成回调（所有回调执行完毕后触发）
    on_scraped: Option<ScrapedCallback>,

    /// 已访问 URL 集合（防重复）
    visited: Arc<Mutex<std::collections::HashSet<String>>>,

    /// 全局默认请求头
    default_headers: HashMap<String, String>,

    /// 是否自动跟踪链接
    follow_links: bool,

    /// 链接选择器（配合 follow_links 使用）
    link_selector: String,

    /// 链接选择器类型
    link_selector_type: LinkSelectorType,

    /// 最大并发数（0 = 使用默认上限）
    max_concurrency: usize,

    /// follow_links 最大递归深度
    max_depth: usize,

    // ── 以下为 Colly 风格新增字段 ──

    /// 按域名限速规则
    limit_rules: Vec<LimitRule>,

    /// 按域名的并发信号量（延迟初始化）
    domain_semaphores: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,

    /// 按域名的最后请求时间
    last_request_time: Arc<Mutex<HashMap<String, Instant>>>,

    /// URL 白名单正则（空 = 不限制）
    url_filters: Vec<Regex>,

    /// URL 黑名单正则（空 = 不限制）
    disallowed_url_filters: Vec<Regex>,

    /// 域名白名单（空 = 不限制）
    allowed_domains: Vec<String>,

    /// 域名黑名单（空 = 不限制）
    disallowed_domains: Vec<String>,
}

impl Collector {
    /// 使用默认后端构建 Collector
    ///
    /// 等价于 [`Collector::reqwest()`]。需要启用 `fetcher-reqwest` feature（默认启用）。
    #[cfg(feature = "fetcher-reqwest")]
    pub fn new() -> Self {
        Self::reqwest()
    }
}

#[cfg(feature = "fetcher-reqwest")]
impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {

    /// 使用 reqwest 后端构建 Collector
    ///
    /// 需要启用 `fetcher-reqwest` feature（默认启用）。
    #[cfg(feature = "fetcher-reqwest")]
    pub fn reqwest() -> Self {
        Self::with_client(ReqwestClient::new())
    }

    /// 使用 wreq 后端构建 Collector（TLS 指纹模拟）
    ///
    /// 需要启用 `fetcher-wreq` feature。
    #[cfg(feature = "fetcher-wreq")]
    pub fn wreq() -> Self {
        Self::with_client(WreqClient::new())
    }

    /// 使用自定义 HTTP 客户端构建 Collector
    ///
    /// 实现 `HttpClient` trait 后传入即可，不依赖任何 feature。
    /// 适用于 Mock 客户端、Chrome CDP 客户端等自定义场景。
    pub fn with_client(client: impl HttpClient + 'static) -> Self {
        Self {
            http_client: Arc::new(client),
            on_request: None,
            on_response_headers: None,
            on_response: None,
            on_html: None,
            on_error: None,
            on_html_elements: Vec::new(),
            on_xml_elements: Vec::new(),
            on_scraped: None,
            visited: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_headers: HashMap::new(),
            follow_links: false,
            link_selector: "a[href]".into(),
            link_selector_type: LinkSelectorType::Css,
            max_concurrency: 0,
            max_depth: DEFAULT_MAX_DEPTH,
            limit_rules: Vec::new(),
            domain_semaphores: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(HashMap::new())),
            url_filters: Vec::new(),
            disallowed_url_filters: Vec::new(),
            allowed_domains: Vec::new(),
            disallowed_domains: Vec::new(),
        }
    }

    /// 设置全局默认请求头
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.default_headers
            .insert(key.to_string(), value.to_string());
    }

    /// 设置最大并发数
    ///
    /// 设为 0 表示使用默认上限（16）。
    pub fn set_max_concurrency(&mut self, n: usize) {
        self.max_concurrency = n;
    }

    /// 启用/禁用链接跟踪
    pub fn set_follow_links(&mut self, follow: bool) {
        self.follow_links = follow;
    }

    /// 设置 follow_links 最大递归深度（默认 10）
    pub fn set_max_depth(&mut self, depth: usize) {
        self.max_depth = depth;
    }

    /// 自定义链接跟踪选择器
    pub fn set_link_selector(&mut self, selector: &str) {
        self.link_selector = selector.to_string();
        self.link_selector_type = LinkSelectorType::Css;
    }

    /// 自定义 XPath 链接跟踪选择器
    pub fn set_link_xpath(&mut self, selector: &str) {
        self.link_selector = selector.to_string();
        self.link_selector_type = LinkSelectorType::Xpath;
    }

    /// 设置链接选择器和类型
    pub fn set_link_selector_with_type(&mut self, selector: &str, selector_type: LinkSelectorType) {
        self.link_selector = selector.to_string();
        self.link_selector_type = selector_type;
    }

    // ──────────────────────────────────────────────
    // 回调注册
    // ──────────────────────────────────────────────

    /// 注册请求前回调
    ///
    /// 在每次 HTTP 请求前调用，可用于修改请求头、打印日志等。
    pub fn on_request(&mut self, callback: impl Fn(&mut Request) + Send + Sync + 'static) {
        self.on_request = Some(Arc::new(callback));
    }

    /// 注册响应回调
    ///
    /// 收到 HTTP 响应后调用，可用于记录状态码等。
    pub fn on_response(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_response = Some(Arc::new(callback));
    }

    /// 注册 HTML 回调
    ///
    /// 解析到 HTML 内容后调用，可用于提取链接或文章内容。
    pub fn on_html(&mut self, callback: impl Fn(&str, &str) + Send + Sync + 'static) {
        self.on_html = Some(Arc::new(callback));
    }

    /// 注册响应头回调
    ///
    /// 收到 HTTP 响应后立即调用（早于 `on_response`），可用于检查状态码、响应头等。
    pub fn on_response_headers(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_response_headers = Some(Arc::new(callback));
    }

    /// 注册 HTML 元素回调（CSS 选择器匹配）
    ///
    /// 当页面中存在匹配 CSS 选择器的元素时，对每个匹配元素调用回调。
    ///
    /// # 示例
    /// ```rust,no_run
    /// use crawlkit::Collector;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut c = Collector::new();
    ///     c.on_html_element("a[href]", |e| {
    ///         if let Some(href) = e.attr("href") {
    ///             println!("链接: {} → {}", e.text(), href);
    ///         }
    ///     });
    ///     c.visit("https://example.com").await.unwrap();
    /// }
    /// ```
    pub fn on_html_element(
        &mut self,
        selector: &str,
        callback: impl Fn(&Element) + Send + Sync + 'static,
    ) {
        self.on_html_elements
            .push((selector.to_string(), Arc::new(callback)));
    }

    /// 注册 XML 元素回调（XPath 匹配）
    ///
    /// 当页面中存在匹配 XPath 表达式的元素时，对每个匹配元素调用回调。
    ///
    /// # 示例
    /// ```rust,no_run
    /// use crawlkit::Collector;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut c = Collector::new();
    ///     c.on_xml_element("//a/@href", |e| {
    ///         println!("href: {:?}", e.text());
    ///     });
    ///     c.visit("https://example.com").await.unwrap();
    /// }
    /// ```
    pub fn on_xml_element(
        &mut self,
        xpath: &str,
        callback: impl Fn(&Element) + Send + Sync + 'static,
    ) {
        self.on_xml_elements
            .push((xpath.to_string(), Arc::new(callback)));
    }

    /// 注销 CSS 选择器回调（Colly 风格的 OnHTMLDetach）
    ///
    /// 移除之前通过 `on_html_element` 注册的匹配指定选择器的回调。
    pub fn on_html_detach(&mut self, selector: &str) {
        self.on_html_elements.retain(|(s, _)| s != selector);
    }

    /// 注销 XPath 回调（Colly 风格的 OnXMLDetach）
    ///
    /// 移除之前通过 `on_xml_element` 注册的匹配指定 XPath 的回调。
    pub fn on_xml_detach(&mut self, xpath: &str) {
        self.on_xml_elements.retain(|(s, _)| s != xpath);
    }

    /// 注册抓取完成回调
    ///
    /// 在所有回调执行完毕后触发，可用于统计、清理等收尾操作。
    pub fn on_scraped(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_scraped = Some(Arc::new(callback));
    }

    /// 注册错误回调
    pub fn on_error(&mut self, callback: impl Fn(&dyn std::error::Error) + Send + Sync + 'static) {
        self.on_error = Some(Arc::new(callback));
    }

    // ──────────────────────────────────────────────
    // 核心爬取方法
    // ──────────────────────────────────────────────

    /// 访问指定 URL
    ///
    /// 流程：构造 Request → 执行 on_request → 发送 HTTP 请求
    /// → 执行 on_response → 如果是 HTML 则执行 on_html
    /// → 若启用 follow_links 则递归访问提取的链接
    pub async fn visit(&self, url: &str) -> Result<()> {
        debug!(url, "开始访问");

        // URL 过滤
        if !self.is_url_allowed(url) {
            debug!("URL 被过滤器拦截: {url}");
            return Ok(());
        }
        if !self.is_domain_allowed(url) {
            debug!("域名不在白名单中: {url}");
            return Ok(());
        }

        let mut req = Request::get(url);
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }

        self.do_request(&mut req, 0).await
    }

    /// 内部请求执行核心逻辑
    #[instrument(skip(self, req), fields(url = %req.url))]
    async fn do_request(&self, req: &mut Request, depth: usize) -> Result<()> {
        // 检查递归深度
        if depth > self.max_depth {
            warn!(url = %req.url, depth, max_depth = self.max_depth, "达到最大递归深度，跳过");
            return Ok(());
        }

        // URL 过滤（递归子链接也检查）
        if !self.is_url_allowed(&req.url) {
            debug!("URL 被过滤器拦截: {}", req.url);
            return Ok(());
        }
        if !self.is_domain_allowed(&req.url) {
            debug!("域名不在白名单中: {}", req.url);
            return Ok(());
        }

        // 检查是否已访问
        {
            let visited = self
                .visited
                .lock()
                .map_err(|e| CrawlError::Lock(format!("锁中毒: {e}")))?;
            if visited.contains(&req.url) && !req.allow_revisit {
                debug!("跳过已访问的 URL");
                return Ok(());
            }
        }

        // 合并客户端默认请求头（如 User-Agent），确保 on_request 回调可见
        for (k, v) in self.http_client.default_headers() {
            req.headers.entry(k).or_insert(v);
        }

        // 限速等待（按域名延迟 + 并发信号量）
        self.enforce_limit(&req.url).await;

        // 执行 on_request 回调
        if let Some(ref cb) = self.on_request {
            debug!("执行 on_request 回调");
            cb(req);
        }

        // 检查是否被回调中止
        if req.aborted {
            debug!(url = %req.url, "请求已被 on_request 回调中止");
            return Ok(());
        }

        // 发送 HTTP 请求
        debug!("发送 HTTP 请求");
        let response = match req.method.as_str() {
            "POST" => self.http_client.post(&req.url, &req.headers, req.body.clone()).await,
            _ => self.http_client.get(&req.url, &req.headers).await,
        };
        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                warn!(error = %e, "HTTP 请求失败");
                if let Some(ref cb) = self.on_error {
                    cb(&e);
                }
                return Err(e);
            }
        };

        debug!(status = response.status, body_len = response.body.len(), "收到响应");

        // 标记已访问
        {
            let mut visited = self
                .visited
                .lock()
                .map_err(|e| CrawlError::Lock(format!("锁中毒: {e}")))?;
            visited.insert(req.url.clone());
        }

        // 执行 on_response_headers 回调（早于 on_response）
        if let Some(ref cb) = self.on_response_headers {
            debug!("执行 on_response_headers 回调");
            cb(&response);
        }

        // 执行 on_response 回调
        if let Some(ref cb) = self.on_response {
            debug!("执行 on_response 回调");
            cb(&response);
        }

        // HTML 内容处理
        if response.is_html() {
            if let Some(ref cb) = self.on_html {
                debug!("执行 on_html 回调");
                cb(&response.body, &response.url);
            }

            // on_html_elements: CSS 选择器匹配
            if !self.on_html_elements.is_empty() {
                let document = Html::parse_document(&response.body);
                for (selector_str, cb) in &self.on_html_elements {
                    match Selector::parse(selector_str) {
                        Ok(sel) => {
                            let matches: Vec<_> = document.select(&sel).collect();
                            debug!(selector = %selector_str, count = matches.len(), "on_html_elements 匹配");
                            for (idx, element_ref) in matches.iter().enumerate() {
                                let name = element_ref.value().name.local.to_string();
                                let text: String = element_ref
                                    .text()
                                    .collect::<Vec<_>>()
                                    .join("")
                                    .trim()
                                    .to_string();
                                let attrs: HashMap<String, String> = element_ref
                                    .value()
                                    .attrs
                                    .iter()
                                    .map(|(k, v)| (k.local.to_string(), v.to_string()))
                                    .collect();
                                let html_str = element_ref.html();
                                let element = Element::new(
                                    name, &response.url, text, attrs, html_str, idx,
                                );
                                cb(&element);
                            }
                        }
                        Err(e) => {
                            warn!(selector = %selector_str, error = %e, "CSS 选择器解析失败");
                        }
                    }
                }
            }

            // on_xml_elements: XPath 匹配
            if !self.on_xml_elements.is_empty() {
                let sanitized = sanitize_for_xpath(&response.body);
                match xpath_html::parse(&sanitized) {
                    Ok(doc) => {
                        let tree = XpathItemTree::from(&doc);
                        for (xpath_expr_str, cb) in &self.on_xml_elements {
                            match skyscraper_xpath::parse(xpath_expr_str) {
                                Ok(xpath_expr) => {
                                    match xpath_expr.apply(&tree) {
                                        Ok(item_set) => {
                                            debug!(xpath = %xpath_expr_str, count = item_set.len(), "on_xml_elements 匹配");
                                            for (idx, item) in item_set.iter().enumerate() {
                                                let element = xpath_item_to_element(
                                                    item,
                                                    &tree,
                                                    &response.url,
                                                    idx,
                                                );
                                                if let Some(el) = element {
                                                    cb(&el);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!(xpath = %xpath_expr_str, error = %e, "XPath 执行失败");
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(xpath = %xpath_expr_str, error = %e, "XPath 解析失败");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "HTML 解析失败（用于 XPath）");
                    }
                }
            }

            // 链接跟踪
            if self.follow_links {
                let abs_links = extract_absolute_links(
                    &response.body,
                    &self.link_selector,
                    self.link_selector_type,
                    &response.url,
                )?;

                debug!(count = abs_links.len(), depth, "提取到子链接，递归访问");
                for link in abs_links {
                    if !self.is_url_allowed(&link) || !self.is_domain_allowed(&link) {
                        continue;
                    }
                    let mut child_req = Request::get(&link);
                    for (k, v) in &self.default_headers {
                        child_req.headers.insert(k.clone(), v.clone());
                    }
                    Box::pin(self.do_request(&mut child_req, depth + 1)).await?;
                }
            }
        }

        // 执行 on_scraped 回调
        if let Some(ref cb) = self.on_scraped {
            debug!("执行 on_scraped 回调");
            cb(&response);
        }

        Ok(())
    }
}

// ── 内部辅助方法 ──

impl Collector {
    /// URL 过滤器检查
    fn is_url_allowed(&self, url: &str) -> bool {
        // 黑名单优先
        for filter in &self.disallowed_url_filters {
            if filter.is_match(url) {
                debug!(url, filter = %filter.as_str(), "URL 匹配黑名单");
                return false;
            }
        }
        // 白名单：非空时才检查
        if !self.url_filters.is_empty() {
            for filter in &self.url_filters {
                if filter.is_match(url) {
                    return true;
                }
            }
            debug!(url, "URL 不匹配任何白名单规则");
            return false;
        }
        true
    }

    /// 域名过滤器检查
    fn is_domain_allowed(&self, url: &str) -> bool {
        let domain = match url::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from)) {
            Some(d) => d,
            None => return true,
        };

        // 黑名单
        for d in &self.disallowed_domains {
            if domain == *d || domain.ends_with(&format!(".{d}")) {
                debug!(domain, disallowed = %d, "域名在黑名单中");
                return false;
            }
        }

        // 白名单：非空时才检查
        if !self.allowed_domains.is_empty() {
            for d in &self.allowed_domains {
                if domain == *d || domain.ends_with(&format!(".{d}")) {
                    return true;
                }
            }
            debug!(domain, "域名不在白名单中");
            return false;
        }

        true
    }

    /// 查找匹配的限速规则
    fn find_matching_rule(&self, domain: &str) -> Option<&LimitRule> {
        self.limit_rules.iter().find(|r| r.matches(domain))
    }

    /// 限速等待（按域名延迟 + 并发信号量）
    async fn enforce_limit(&self, url: &str) {
        let domain = match url::Url::parse(url).ok().and_then(|u| u.host_str().map(String::from)) {
            Some(d) => d,
            None => return,
        };

        // 计算需要等待的延迟（锁在计算后立即释放）
        let delay = self
            .last_request_time
            .lock()
            .map_err(|e| warn!(error = %e, "last_request_time 锁中毒"))
            .ok()
            .and_then(|guard| {
                guard.get(&domain).and_then(|last| {
                    let elapsed = last.elapsed();
                    self.find_matching_rule(&domain).and_then(|rule| {
                        if elapsed < rule.delay {
                            let mut d = rule.delay.checked_sub(elapsed)?;
                            if rule.random_delay > Duration::ZERO {
                                let extra =
                                    rand::random::<f64>() * rule.random_delay.as_secs_f64();
                                d += Duration::from_secs_f64(extra);
                            }
                            Some(d)
                        } else {
                            None
                        }
                    })
                })
            });
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }

        // 更新最后请求时间
        if let Ok(mut guard) = self.last_request_time.lock() {
            guard.insert(domain.clone(), Instant::now());
        }

        // 并发信号量（锁在 await 前释放）
        let parallelism = self
            .find_matching_rule(&domain)
            .map_or(1, |r| r.parallelism.max(1));
        let sem = match self.domain_semaphores.lock() {
            Ok(mut guard) => guard
                .entry(domain.clone())
                .or_insert_with(|| Arc::new(Semaphore::new(parallelism)))
                .clone(),
            Err(e) => {
                warn!(error = %e, "domain_semaphores 锁中毒");
                return;
            }
        };
        let _permit = sem.acquire().await;
        // permit 在此释放
    }
}

// ── Clone（共享 HTTP 后端 + 回调，不共享 visited/限速状态） ──

impl Clone for Collector {
    fn clone(&self) -> Self {
        Self {
            http_client: self.http_client.clone(),
            on_request: self.on_request.clone(),
            on_response_headers: self.on_response_headers.clone(),
            on_response: self.on_response.clone(),
            on_html: self.on_html.clone(),
            on_error: self.on_error.clone(),
            on_html_elements: self.on_html_elements.clone(),
            on_xml_elements: self.on_xml_elements.clone(),
            on_scraped: self.on_scraped.clone(),
            visited: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_headers: self.default_headers.clone(),
            follow_links: self.follow_links,
            link_selector: self.link_selector.clone(),
            link_selector_type: self.link_selector_type,
            max_concurrency: self.max_concurrency,
            max_depth: self.max_depth,
            limit_rules: self.limit_rules.clone(),
            domain_semaphores: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(HashMap::new())),
            url_filters: self.url_filters.clone(),
            disallowed_url_filters: self.disallowed_url_filters.clone(),
            allowed_domains: self.allowed_domains.clone(),
            disallowed_domains: self.disallowed_domains.clone(),
        }
    }
}

// ── Colly 风格新增方法 ──

impl Collector {
    /// 创建 Collector 的配置副本（共享 HTTP 后端，不复制回调，独立 visited 集）
    ///
    /// 相当于 Colly 的 `Clone()` 方法的无回调版本。可用于多 Collector 协作场景：
    /// 一个爬列表页提取链接，克隆的 Collector 爬详情页。
    pub fn clone_config(&self) -> Self {
        Self {
            http_client: self.http_client.clone(),
            on_request: None,
            on_response_headers: None,
            on_response: None,
            on_html: None,
            on_error: None,
            on_html_elements: Vec::new(),
            on_xml_elements: Vec::new(),
            on_scraped: None,
            visited: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_headers: self.default_headers.clone(),
            follow_links: self.follow_links,
            link_selector: self.link_selector.clone(),
            link_selector_type: self.link_selector_type,
            max_concurrency: self.max_concurrency,
            max_depth: self.max_depth,
            limit_rules: self.limit_rules.clone(),
            domain_semaphores: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(HashMap::new())),
            url_filters: self.url_filters.clone(),
            disallowed_url_filters: self.disallowed_url_filters.clone(),
            allowed_domains: self.allowed_domains.clone(),
            disallowed_domains: self.disallowed_domains.clone(),
        }
    }

    /// 添加限速规则
    ///
    /// 按域名通配符控制并发数和请求间隔。
    /// 多个规则各自独立生效。
    pub fn add_limit(&mut self, rule: LimitRule) {
        self.limit_rules.push(rule);
    }

    /// 添加多个限速规则
    pub fn add_limits(&mut self, rules: Vec<LimitRule>) {
        self.limit_rules.extend(rules);
    }

    /// 添加 URL 白名单正则
    ///
    /// 设置后只有匹配的 URL 才会被爬取。
    /// 非空时才启用过滤。
    pub fn add_url_filter(&mut self, pattern: &str) {
        if let Ok(re) = Regex::new(pattern) {
            self.url_filters.push(re);
        }
    }

    /// 添加 URL 黑名单正则
    ///
    /// 匹配的 URL 将被跳过。
    pub fn add_disallowed_url_filter(&mut self, pattern: &str) {
        if let Ok(re) = Regex::new(pattern) {
            self.disallowed_url_filters.push(re);
        }
    }

    /// 设置域名白名单
    ///
    /// 只有这些域名下的 URL 才会被爬取。
    /// 空列表 = 不限制。
    pub fn set_allowed_domains(&mut self, domains: Vec<String>) {
        self.allowed_domains = domains;
    }

    /// 设置域名黑名单
    ///
    /// 这些域名下的 URL 将被跳过。
    pub fn set_disallowed_domains(&mut self, domains: Vec<String>) {
        self.disallowed_domains = domains;
    }

    /// 并发运行 URL 批处理 —— Colly 风格的 Async + Wait
    ///
    /// 在 `Arc<Collector>` 上调用，通过信号量控制并发（取 LimitRule 中最大 parallelism）。
    /// 所有请求共享 HTTP 后端、**回调**、visited 去重（**`visit()` 触发完整回调链**）。
    /// 结果请通过 `on_response` / `on_scraped` 等回调获取。
    ///
    /// # 示例
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use crawlkit::Collector;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut c = Collector::new();
    ///     c.on_response(|resp| println!("{}: {} bytes", resp.url, resp.body.len()));
    ///     let c = Arc::new(c);
    ///     let urls = vec!["https://example.com".to_string()];
    ///     c.run(urls).await;
    /// }
    /// ```
    pub async fn run(self: Arc<Self>, urls: Vec<String>) {
        let max_parallel = self
            .limit_rules
            .iter()
            .map(|r| r.parallelism)
            .max()
            .unwrap_or(4)
            .max(1);

        let sem = Arc::new(Semaphore::new(max_parallel));
        let mut handles = Vec::with_capacity(urls.len());

        for url in urls {
            let permit = match sem.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    warn!("信号量已关闭，跳过剩余请求");
                    break;
                }
            };
            let c = self.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = c.visit(&url).await {
                    warn!("run 请求失败 [{}]: {e}", url);
                }
                drop(permit);
            }));
        }

        for h in handles {
            let _ = h.await;
        }
    }
}

/// 将 skyscraper XPath 匹配项转为 Element
fn xpath_item_to_element<'a>(
    item: &crawlkit_parser::skyscraper::xpath::grammar::data_model::XpathItem,
    tree: &'a XpathItemTree,
    url: &'a str,
    index: usize,
) -> Option<Element<'a>> {
    use crawlkit_parser::skyscraper::xpath::grammar::data_model::{Node, XpathItem};
    use crawlkit_parser::skyscraper::xpath::grammar::{NonTreeXpathNode, XpathItemTreeNodeData};

    match item {
        XpathItem::Node(Node::TreeNode(tree_node)) => match tree_node.data {
            XpathItemTreeNodeData::ElementNode(element) => {
                let name = element.name.clone();
                let mut attrs = HashMap::new();
                for attr in &element.attributes {
                    attrs.insert(attr.name.clone(), attr.value.clone());
                }
                let text = tree_node.all_text(tree).trim().to_string();
                let html_str = element.to_string();
                Some(Element::new(name, url, text, attrs, html_str, index))
            }
            _ => None,
        },
        XpathItem::Node(Node::NonTreeNode(NonTreeXpathNode::AttributeNode(attr))) => {
            let mut attrs = HashMap::new();
            attrs.insert(attr.name.clone(), attr.value.clone());
            Some(Element::new(
                attr.name.clone(),
                url,
                attr.value.clone(),
                attrs,
                format!("{}=\"{}\"", attr.name, attr.value),
                index,
            ))
        }
        _ => None,
    }
}

impl Collector {
    // ──────────────────────────────────────────────
    // 便捷方法
    // ──────────────────────────────────────────────

    /// 提取页面中所有匹配的链接
    ///
    /// 不触发回调，直接返回绝对 URL 列表。
    pub async fn get_links(&self, url: &str, selector: &str) -> Result<Vec<String>> {
        debug!(url, selector, "提取链接");
        let mut req = Request::get(url);
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }
        for (k, v) in self.http_client.default_headers() {
            req.headers.entry(k).or_insert(v);
        }
        let response = self.http_client.get(url, &req.headers).await?;
        let links = extract_absolute_links(
            &response.body,
            selector,
            LinkSelectorType::Css,
            &response.url,
        )?;
        debug!(count = links.len(), "提取到链接");
        Ok(links)
    }

    /// 使用 XPath 提取页面中所有匹配的链接
    ///
    /// 不触发回调，直接返回绝对 URL 列表。
    pub async fn get_links_by_xpath(&self, url: &str, selector: &str) -> Result<Vec<String>> {
        debug!(url, selector, "使用 XPath 提取链接");
        let mut req = Request::get(url);
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }
        for (k, v) in self.http_client.default_headers() {
            req.headers.entry(k).or_insert(v);
        }
        let response = self.http_client.get(url, &req.headers).await?;
        let links = extract_absolute_links(
            &response.body,
            selector,
            LinkSelectorType::Xpath,
            &response.url,
        )?;
        debug!(count = links.len(), "提取到链接");
        Ok(links)
    }

    /// 一步提取文章内容
    ///
    /// 不触发回调，直接返回 Article 结构。
    pub async fn get_article(&self, url: &str) -> Result<Article> {
        debug!(url, "提取文章");
        let mut headers = self.default_headers.clone();
        for (k, v) in self.http_client.default_headers() {
            headers.entry(k).or_insert(v);
        }
        let response = self.http_client.get(url, &headers).await?;
        let article = extract_article(&response.body, &response.url);
        debug!(title = %article.title, "文章提取完成");
        Ok(article)
    }

    /// 批量并发抓取文章
    pub async fn get_articles(&self, urls: &[String]) -> Vec<Result<Article>> {
        debug!(count = urls.len(), "开始批量抓取文章");
        let mut handles = Vec::new();
        let client = self.http_client.clone();
        let mut headers = self.default_headers.clone();
        for (k, v) in self.http_client.default_headers() {
            headers.entry(k).or_insert(v);
        }
        let concurrency = if self.max_concurrency == 0 {
            urls.len().min(DEFAULT_MAX_CONCURRENCY)
        } else {
            self.max_concurrency
        };

        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

        for url in urls {
            let url = url.clone();
            let client = client.clone();
            let headers = headers.clone();
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    continue;
                }
            };
            handles.push(tokio::spawn(async move {
                let result = client.get(&url, &headers).await;
                drop(permit);
                match result {
                    Ok(resp) => Ok(extract_article(&resp.body, &resp.url)),
                    Err(e) => Err(e),
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(r) => results.push(r),
                Err(e) => results.push(Err(CrawlError::Http(format!("任务 panicked: {e}")))),
            }
        }
        let success = results.iter().filter(|r| r.is_ok()).count();
        debug!(total = urls.len(), success, "批量抓取完成");
        results
    }

    /// 获取底层 HTTP 客户端引用
    pub fn client(&self) -> &dyn HttpClient {
        self.http_client.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockClient {
        responses: Vec<Result<Response>>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockClient {
        fn ok(body: &str) -> Self {
            Self {
                responses: vec![Ok(Response {
                    url: "http://test.com".into(),
                    status: 200,
                    headers: HashMap::from([("content-type".into(), "text/html".into())]),
                    body: body.to_string(),
                })],
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn fail() -> Self {
            Self {
                responses: vec![Err(CrawlError::Http("模拟错误".into()))],
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn sequence(responses: Vec<Result<Response>>) -> Self {
            Self {
                responses,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockClient {
        async fn get(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<Response> {
            let idx = self.call_count.fetch_add(1, Ordering::Relaxed);
            match self.responses.get(idx) {
                Some(Ok(r)) => Ok(r.clone()),
                Some(Err(e)) => Err(CrawlError::Http(e.to_string())),
                None => Err(CrawlError::Http("无更多模拟响应".into())),
            }
        }

        async fn post(
            &self,
            url: &str,
            headers: &HashMap<String, String>,
            _body: Vec<u8>,
        ) -> Result<Response> {
            self.get(url, headers).await
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_visit_basic() {
        let c = Collector::with_client(MockClient::ok("<html></html>"));
        let result = c.visit("http://test.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_visit_error() {
        let c = Collector::with_client(MockClient::fail());
        let result = c.visit("http://test.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_visited_dedup() {
        let client = MockClient::sequence(vec![
            Ok(Response {
                url: "http://test.com".into(),
                status: 200,
                headers: HashMap::new(),
                body: "ok".into(),
            }),
            Ok(Response {
                url: "http://test.com".into(),
                status: 200,
                headers: HashMap::new(),
                body: "ok2".into(),
            }),
        ]);
        let call_count = Arc::clone(&client.call_count);
        let c = Collector::with_client(client);

        c.visit("http://test.com").await.unwrap();
        c.visit("http://test.com").await.unwrap();

        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_on_request_callback() {
        use std::sync::atomic::AtomicBool;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let mut c = Collector::with_client(MockClient::ok("<html></html>"));
        c.on_request(move |_req| {
            called_clone.store(true, Ordering::Relaxed);
        });
        c.visit("http://test.com").await.unwrap();
        assert!(called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_max_depth_limits_recursion() {
        // 每个页面都包含子链接，用于验证深度限制
        let root_html = r#"<html><body>
            <a href="http://test.com/a">link1</a>
            <a href="http://test.com/b">link2</a>
        </body></html>"#;
        let child_html = r#"<html><body>
            <a href="http://test.com/c">deep link</a>
        </body></html>"#;
        // /c 页面也包含链接，但 depth=2 时不应被访问
        let deep_html = r#"<html><body>
            <a href="http://test.com/d">very deep</a>
        </body></html>"#;

        let client = MockClient::sequence(vec![
            // depth 0: root
            Ok(Response {
                url: "http://test.com".into(),
                status: 200,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: root_html.to_string(),
            }),
            // depth 1: /a (包含子链接)
            Ok(Response {
                url: "http://test.com/a".into(),
                status: 200,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: child_html.to_string(),
            }),
            // depth 1: /b (包含子链接)
            Ok(Response {
                url: "http://test.com/b".into(),
                status: 200,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: child_html.to_string(),
            }),
            // depth 2 的 /c 不应被访问，所以不需要准备响应
            // 如果 depth 2 被执行，MockClient 会返回 "无更多模拟响应" 错误
            Ok(Response {
                url: "http://test.com/c".into(),
                status: 200,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: deep_html.to_string(),
            }),
        ]);
        let call_count = Arc::clone(&client.call_count);
        let mut c = Collector::with_client(client);
        c.set_follow_links(true);
        c.set_max_depth(1);

        c.visit("http://test.com").await.unwrap();

        // depth 0: root (1 request)
        // depth 1: /a, /b (2 requests)
        // depth 2: /c, /d 被 max_depth 阻止
        assert_eq!(call_count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_on_response_headers_callback() {
        use std::sync::atomic::AtomicBool;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let mut c = Collector::with_client(MockClient::ok("<html></html>"));
        c.on_response_headers(move |_resp| {
            called_clone.store(true, Ordering::Relaxed);
        });
        c.visit("http://test.com").await.unwrap();
        assert!(called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_on_html_element_callback() {
        let hit = Arc::new(Mutex::new(Vec::<String>::new()));
        let hit_clone = Arc::clone(&hit);

        let html = r#"<html><body>
            <a href="/a" class="link">Link A</a>
            <a href="/b" class="link">Link B</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("a.link", move |e| {
            hit_clone.lock().unwrap().push(e.text().to_string());
        });
        c.visit("http://test.com").await.unwrap();

        let texts = hit.lock().unwrap();
        assert_eq!(texts.len(), 2);
        assert!(texts.contains(&"Link A".to_string()));
        assert!(texts.contains(&"Link B".to_string()));
    }

    #[tokio::test]
    async fn test_on_html_element_attr_access() {
        let hrefs = Arc::new(Mutex::new(Vec::<String>::new()));
        let hrefs_clone = Arc::clone(&hrefs);

        let html = r#"<html><body>
            <a href="http://example.com/1">First</a>
            <a href="http://example.com/2">Second</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("a", move |e| {
            if let Some(href) = e.attr("href") {
                hrefs_clone.lock().unwrap().push(href.to_string());
            }
        });
        c.visit("http://test.com").await.unwrap();

        let hrefs = hrefs.lock().unwrap();
        assert_eq!(hrefs.len(), 2);
        assert!(hrefs.contains(&"http://example.com/1".to_string()));
        assert!(hrefs.contains(&"http://example.com/2".to_string()));
    }

    #[tokio::test]
    async fn test_on_xml_element_callback() {
        let names = Arc::new(Mutex::new(Vec::<String>::new()));
        let names_clone = Arc::clone(&names);

        let html = r#"<html><body>
            <item><name>Apple</name></item>
            <item><name>Banana</name></item>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_xml_element("//name", move |e| {
            names_clone.lock().unwrap().push(e.text().to_string());
        });
        c.visit("http://test.com").await.unwrap();

        let names = names.lock().unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Apple".to_string()));
        assert!(names.contains(&"Banana".to_string()));
    }

    #[tokio::test]
    async fn test_on_scraped_callback() {
        use std::sync::atomic::AtomicBool;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        let mut c = Collector::with_client(MockClient::ok("<html></html>"));
        c.on_scraped(move |_resp| {
            called_clone.store(true, Ordering::Relaxed);
        });
        c.visit("http://test.com").await.unwrap();
        assert!(called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_on_response_headers_before_on_response() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let order = Arc::new(AtomicUsize::new(0));
        let order_clone = Arc::clone(&order);

        let mut c = Collector::with_client(MockClient::ok("<html></html>"));
        c.on_response_headers(move |_resp| {
            order_clone.fetch_add(1, Ordering::Relaxed);
        });
        let order_clone2 = Arc::clone(&order);
        c.on_response(move |_resp| {
            // on_response should see value 1 (on_response_headers ran first)
            let val = order_clone2.fetch_add(10, Ordering::Relaxed);
            assert_eq!(val, 1, "on_response_headers 应先于 on_response 执行");
        });
        c.visit("http://test.com").await.unwrap();
    }

    #[tokio::test]
    async fn test_element_name_and_index() {
        let results = Arc::new(Mutex::new(Vec::<(String, usize, String)>::new()));
        let results_clone = Arc::clone(&results);

        let html = r#"<html><body>
            <a href="/1">First</a>
            <a href="/2">Second</a>
            <a href="/3">Third</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("a", move |e| {
            results_clone.lock().unwrap().push((
                e.name.clone(),
                e.index,
                e.text().to_string(),
            ));
        });
        c.visit("http://test.com").await.unwrap();

        let results = results.lock().unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], ("a".to_string(), 0, "First".to_string()));
        assert_eq!(results[1], ("a".to_string(), 1, "Second".to_string()));
        assert_eq!(results[2], ("a".to_string(), 2, "Third".to_string()));
    }

    #[tokio::test]
    async fn test_element_child_text() {
        let result = Arc::new(Mutex::new(String::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<html><body>
            <div class="card">
                <h2>Title</h2>
                <p>Content here</p>
            </div>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("div.card", move |e| {
            let title = e.child_text("h2");
            let content = e.child_text("p");
            *result_clone.lock().unwrap() = format!("{}|{}", title, content);
        });
        c.visit("http://test.com").await.unwrap();

        assert_eq!(*result.lock().unwrap(), "Title|Content here");
    }

    #[tokio::test]
    async fn test_element_child_attr() {
        let result = Arc::new(Mutex::new(Vec::<String>::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<html><body>
            <div class="links">
                <a href="/page1">Page 1</a>
                <a href="/page2">Page 2</a>
            </div>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("div.links", move |e| {
            let hrefs = e.child_attrs("a", "href");
            *result_clone.lock().unwrap() = hrefs;
        });
        c.visit("http://test.com").await.unwrap();

        let hrefs = result.lock().unwrap();
        assert_eq!(hrefs.len(), 2);
        assert_eq!(hrefs[0], "/page1");
        assert_eq!(hrefs[1], "/page2");
    }

    #[tokio::test]
    async fn test_element_attrs_iterator() {
        let result = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<html><body>
            <a href="/link" class="nav" id="main">Click</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("a", move |e| {
            let attrs: Vec<_> = e.attrs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            *result_clone.lock().unwrap() = attrs;
        });
        c.visit("http://test.com").await.unwrap();

        let attrs = result.lock().unwrap();
        assert_eq!(attrs.len(), 3);
        assert!(attrs.iter().any(|(k, v)| k == "href" && v == "/link"));
        assert!(attrs.iter().any(|(k, v)| k == "class" && v == "nav"));
        assert!(attrs.iter().any(|(k, v)| k == "id" && v == "main"));
    }

    #[tokio::test]
    async fn test_element_absolute_url() {
        let result = Arc::new(Mutex::new(Vec::<String>::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<html><body>
            <a href="/docs/">Docs</a>
            <a href="https://github.com/user/repo">GitHub</a>
            <a href="../relative">Relative</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_html_element("a", move |e| {
            if let Some(abs) = e.absolute_url("href") {
                result_clone.lock().unwrap().push(abs);
            }
        });
        c.visit("http://test.com").await.unwrap();

        let urls = result.lock().unwrap();
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "http://test.com/docs/");
        assert_eq!(urls[1], "https://github.com/user/repo");
        assert_eq!(urls[2], "http://test.com/relative");
    }

    #[tokio::test]
    async fn test_on_xml_element_has_href() {
        let result = Arc::new(Mutex::new(Vec::<(String, String, String)>::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<html><body>
            <a href="/docs/">Docs</a>
            <a href="/articles/">Articles</a>
        </body></html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_xml_element("//a", move |e| {
            let href = e.attr("href").unwrap_or("").to_string();
            let abs = e.absolute_url("href").unwrap_or_default();
            let text = e.text().to_string();
            result_clone.lock().unwrap().push((href, abs, text));
        });
        c.visit("http://test.com").await.unwrap();

        let results = result.lock().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("/docs/".to_string(), "http://test.com/docs/".to_string(), "Docs".to_string()));
        assert_eq!(results[1], ("/articles/".to_string(), "http://test.com/articles/".to_string(), "Articles".to_string()));
    }

    #[tokio::test]
    async fn test_on_xml_element_realistic_html() {
        let result = Arc::new(Mutex::new(Vec::<String>::new()));
        let result_clone = Arc::clone(&result);

        let html = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Test</title></head>
<body>
  <header>
    <nav>
      <a href="/docs/">Docs</a>
      <a href="/articles/">Articles</a>
    </nav>
  </header>
  <main>
    <p>Content</p>
  </main>
</body>
</html>"#;
        let mut c = Collector::with_client(MockClient::ok(html));
        c.on_xml_element("//a", move |e| {
            let href = e.attr("href").unwrap_or("").to_string();
            let text = e.text().to_string();
            result_clone.lock().unwrap().push(format!("{}:{}", href, text));
        });
        c.visit("http://test.com").await.unwrap();

        let results = result.lock().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "/docs/:Docs");
        assert_eq!(results[1], "/articles/:Articles");
    }
}
