//! Collector 核心调度器
//!
//! 负责管理回调链、调度 HTTP 请求、跟踪已访问 URL 防止重复。
//! 设计参考 Go colly 的 Collector，采用 Builder + 回调模式。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tracing::{debug, instrument, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::request::Request;
use crawlkit_core::response::Response;
#[cfg(feature = "fetcher-reqwest")]
use crawlkit_fetcher_reqwest::ReqwestClient;
#[cfg(feature = "fetcher-wreq")]
use crawlkit_fetcher_wreq::WreqClient;
use crawlkit_parser::html::{extract_absolute_links, extract_article, Article, LinkSelectorType};

/// 回调函数类型别名
type RequestCallback = Box<dyn Fn(&mut Request) + Send + Sync>;
type ResponseCallback = Box<dyn Fn(&Response) + Send + Sync>;
type HtmlCallback = Box<dyn Fn(&str, &str) + Send + Sync>;
type ErrorCallback = Box<dyn Fn(&dyn std::error::Error) + Send + Sync>;

/// 爬虫收集器 —— 框架核心调度器
///
/// # 使用示例
/// ```rust,no_run
/// use crawlkit::Collector;
///
/// #[tokio::main]
/// async fn main() {
///     let mut c = Collector::reqwest();
///     c.on_request(|req| println!("请求: {}", req.url));
///     c.visit("https://example.com").await.unwrap();
/// }
/// ```
pub struct Collector {
    /// HTTP 客户端（可替换）
    http_client: Arc<dyn HttpClient>,

    /// 请求前回调
    on_request: Option<RequestCallback>,

    /// 收到响应后回调
    on_response: Option<ResponseCallback>,

    /// HTML 解析后回调
    on_html: Option<HtmlCallback>,

    /// 错误回调
    on_error: Option<ErrorCallback>,

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

    /// 最大并发数（0 = 不限制）
    max_concurrency: usize,
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
            on_response: None,
            on_html: None,
            on_error: None,
            visited: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_headers: HashMap::new(),
            follow_links: false,
            link_selector: "a[href]".into(),
            link_selector_type: LinkSelectorType::Css,
            max_concurrency: 0,
        }
    }

    /// 设置全局默认请求头
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.default_headers
            .insert(key.to_string(), value.to_string());
    }

    /// 设置最大并发数
    pub fn set_max_concurrency(&mut self, n: usize) {
        self.max_concurrency = n;
    }

    /// 启用/禁用链接跟踪
    pub fn set_follow_links(&mut self, follow: bool) {
        self.follow_links = follow;
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
        self.do_request(&mut req).await
    }

    /// 内部请求执行核心逻辑
    #[instrument(skip(self, req), fields(url = %req.url))]
    async fn do_request(&mut self, req: &mut Request) -> Result<()> {
        // 检查是否已访问
        {
            let visited = self.visited.lock().unwrap();
            if visited.contains(&req.url) && !req.allow_revisit {
                debug!("跳过已访问的 URL");
                return Ok(());
            }
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
                return Err(e.into());
            }
        };

        debug!(status = response.status, body_len = response.body.len(), "收到响应");

        // 标记已访问
        {
            let mut visited = self.visited.lock().unwrap();
            visited.insert(req.url.clone());
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

            // 链接跟踪
            if self.follow_links {
                let abs_links = extract_absolute_links(
                    &response.body,
                    &self.link_selector,
                    self.link_selector_type,
                    &response.url,
                )?;

                debug!(count = abs_links.len(), "提取到子链接，开始递归访问");
                for link in abs_links {
                    let mut child_req = Request::get(&link);
                    for (k, v) in &self.default_headers {
                        child_req.headers.insert(k.clone(), v.clone());
                    }
                    Box::pin(self.do_request(&mut child_req)).await?;
                }
            }
        }

        Ok(())
    }

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
            urls.len()
        } else {
            self.max_concurrency
        };

        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

        for url in urls {
            let url = url.clone();
            let client = client.clone();
            let headers = headers.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            handles.push(tokio::spawn(async move {
                let result = client.get(&url, &headers).await;
                drop(permit);
                match result {
                    Ok(resp) => Ok(extract_article(&resp.body, &resp.url)),
                    Err(e) => Err(anyhow::anyhow!("请求失败: {e}")),
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(r) => results.push(r),
                Err(e) => results.push(Err(anyhow::anyhow!("任务 panicked: {e}"))),
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
