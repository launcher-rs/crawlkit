//! Collector 核心调度器
//!
//! 负责管理回调链、调度 HTTP 请求、跟踪已访问 URL 防止重复。
//! 设计参考 Go colly 的 Collector，采用 Builder + 回调模式。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::{debug, instrument, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::request::Request;
use crawlkit_core::response::Response;
#[cfg(feature = "fetcher-reqwest")]
use crawlkit_fetcher_reqwest::ReqwestClient;
#[cfg(feature = "fetcher-wreq")]
use crawlkit_fetcher_wreq::WreqClient;
use crawlkit_parser::html::{extract_absolute_links, extract_article, Article, LinkSelectorType};
use crawlkit_parser::scraper::{Html, Selector};
use crawlkit_parser::skyscraper::html as xpath_html;
use crawlkit_parser::skyscraper::xpath::{self as skyscraper_xpath, XpathItemTree};

/// follow_links 默认最大递归深度
const DEFAULT_MAX_DEPTH: usize = 10;

/// 默认并发上限（当 max_concurrency 为 0 时使用）
const DEFAULT_MAX_CONCURRENCY: usize = 16;

/// 回调函数类型别名
type RequestCallback = Box<dyn Fn(&mut Request) + Send + Sync>;
type ResponseCallback = Box<dyn Fn(&Response) + Send + Sync>;
type HtmlCallback = Box<dyn Fn(&str, &str) + Send + Sync>;
type ErrorCallback = Box<dyn Fn(&dyn std::error::Error) + Send + Sync>;
type ResponseHeadersCallback = Box<dyn Fn(&Response) + Send + Sync>;
type ScrapedCallback = Box<dyn Fn(&Response) + Send + Sync>;

/// HTML 元素回调（CSS 选择器匹配）
type HtmlElementCallback = Box<dyn Fn(&Element) + Send + Sync>;

/// XML 元素回调（XPath 匹配）
type XmlElementCallback = Box<dyn Fn(&Element) + Send + Sync>;

/// HTML 元素包装器
///
/// 在 `on_html_element` / `on_xml_element` 回调中使用，
/// 提供对匹配元素的文本、属性、HTML 内容的访问。
pub struct Element<'a> {
    /// 当前页面 URL
    pub url: &'a str,
    /// 元素的纯文本内容
    text: String,
    /// 元素属性
    attrs: HashMap<String, String>,
    /// 元素原始 HTML
    html: String,
}

impl<'a> Element<'a> {
    fn new(url: &'a str, text: String, attrs: HashMap<String, String>, html: String) -> Self {
        Self {
            url,
            text,
            attrs,
            html,
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

    /// 获取元素的原始 HTML
    pub fn html(&self) -> &str {
        &self.html
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
}

impl Collector {
    /// 使用默认后端构建 Collector
    ///
    /// 等价于 [`Collector::reqwest()`]。需要启用 `fetcher-reqwest` feature（默认启用）。
    #[cfg(feature = "fetcher-reqwest")]
    pub fn new() -> Self {
        Self::reqwest()
    }

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
        self.on_request = Some(Box::new(callback));
    }

    /// 注册响应回调
    ///
    /// 收到 HTTP 响应后调用，可用于记录状态码等。
    pub fn on_response(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_response = Some(Box::new(callback));
    }

    /// 注册 HTML 回调
    ///
    /// 解析到 HTML 内容后调用，可用于提取链接或文章内容。
    pub fn on_html(&mut self, callback: impl Fn(&str, &str) + Send + Sync + 'static) {
        self.on_html = Some(Box::new(callback));
    }

    /// 注册响应头回调
    ///
    /// 收到 HTTP 响应后立即调用（早于 `on_response`），可用于检查状态码、响应头等。
    pub fn on_response_headers(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_response_headers = Some(Box::new(callback));
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
            .push((selector.to_string(), Box::new(callback)));
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
            .push((xpath.to_string(), Box::new(callback)));
    }

    /// 注册抓取完成回调
    ///
    /// 在所有回调执行完毕后触发，可用于统计、清理等收尾操作。
    pub fn on_scraped(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_scraped = Some(Box::new(callback));
    }

    /// 注册错误回调
    pub fn on_error(&mut self, callback: impl Fn(&dyn std::error::Error) + Send + Sync + 'static) {
        self.on_error = Some(Box::new(callback));
    }

    // ──────────────────────────────────────────────
    // 核心爬取方法
    // ──────────────────────────────────────────────

    /// 访问指定 URL
    ///
    /// 流程：构造 Request → 执行 on_request → 发送 HTTP 请求
    /// → 执行 on_response → 如果是 HTML 则执行 on_html
    /// → 若启用 follow_links 则递归访问提取的链接
    pub async fn visit(&mut self, url: &str) -> Result<()> {
        debug!(url, "开始访问");
        let mut req = Request::get(url);
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }
        self.do_request(&mut req, 0).await
    }

    /// 内部请求执行核心逻辑
    #[instrument(skip(self, req), fields(url = %req.url))]
    async fn do_request(&mut self, req: &mut Request, depth: usize) -> Result<()> {
        // 检查递归深度
        if depth > self.max_depth {
            warn!(url = %req.url, depth, max_depth = self.max_depth, "达到最大递归深度，跳过");
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

        // 执行 on_request 回调
        if let Some(ref cb) = self.on_request {
            debug!("执行 on_request 回调");
            cb(req);
        }

        // 发送 HTTP 请求
        debug!("发送 HTTP 请求");
        let response = match self.http_client.get(&req.url, &req.headers).await {
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
                            for element_ref in &matches {
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
                                let element = Element::new(&req.url, text, attrs, html_str);
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
                match xpath_html::parse(&response.body) {
                    Ok(doc) => {
                        let tree = XpathItemTree::from(&doc);
                        for (xpath_expr_str, cb) in &self.on_xml_elements {
                            match skyscraper_xpath::parse(xpath_expr_str) {
                                Ok(xpath_expr) => {
                                    match xpath_expr.apply(&tree) {
                                        Ok(item_set) => {
                                            debug!(xpath = %xpath_expr_str, count = item_set.len(), "on_xml_elements 匹配");
                                            for item in &item_set {
                                                let element = xpath_item_to_element(
                                                    item,
                                                    &tree,
                                                    &req.url,
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

/// 将 skyscraper XPath 匹配项转为 Element
fn xpath_item_to_element<'a>(
    item: &crawlkit_parser::skyscraper::xpath::grammar::data_model::XpathItem,
    tree: &'a XpathItemTree,
    url: &'a str,
) -> Option<Element<'a>> {
    use crawlkit_parser::skyscraper::xpath::grammar::data_model::{Node, XpathItem};
    use crawlkit_parser::skyscraper::xpath::grammar::{NonTreeXpathNode, XpathItemTreeNodeData};

    match item {
        XpathItem::Node(Node::TreeNode(tree_node)) => match tree_node.data {
            XpathItemTreeNodeData::ElementNode(element) => {
                let mut attrs = HashMap::new();
                for attr in &element.attributes {
                    attrs.insert(attr.name.clone(), attr.value.clone());
                }
                let text = tree_node.all_text(tree).trim().to_string();
                let html_str = element.to_string();
                Some(Element::new(url, text, attrs, html_str))
            }
            _ => None,
        },
        XpathItem::Node(Node::NonTreeNode(NonTreeXpathNode::AttributeNode(attr))) => {
            let mut attrs = HashMap::new();
            attrs.insert(attr.name.clone(), attr.value.clone());
            Some(Element::new(
                url,
                attr.value.clone(),
                attrs,
                format!("{}=\"{}\"", attr.name, attr.value),
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
        let response = self.http_client.get(url, &self.default_headers).await?;
        let article = extract_article(&response.body, &response.url);
        debug!(title = %article.title, "文章提取完成");
        Ok(article)
    }

    /// 批量并发抓取文章
    pub async fn get_articles(&self, urls: &[String]) -> Vec<Result<Article>> {
        debug!(count = urls.len(), "开始批量抓取文章");
        let mut handles = Vec::new();
        let client = self.http_client.clone();
        let headers = self.default_headers.clone();
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
        let mut c = Collector::with_client(MockClient::ok("<html></html>"));
        let result = c.visit("http://test.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_visit_error() {
        let mut c = Collector::with_client(MockClient::fail());
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
        let mut c = Collector::with_client(client);

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
}
