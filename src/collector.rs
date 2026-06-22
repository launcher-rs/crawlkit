//! Collector 核心模块
//!
//! `Collector` 是框架的核心调度器，负责：
//! 1. 管理回调链（OnRequest / OnHTML / OnResponse / OnError）
//! 2. 调度 HTTP 客户端发送请求
//! 3. 跟踪已访问 URL 防止重复
//! 4. 协调整个爬取流程
//!
//! 设计参考 Go colly 的 Collector，但采用 Rust 惯用的 Builder + 回调模式。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::client::{HttpClient, ReqwestClient};
use crate::html::{extract_article, extract_links, resolve_url, Article};
use crate::request::Request;
use crate::response::Response;

/// 回调函数类型别名
type RequestCallback = Box<dyn Fn(&mut Request) + Send + Sync>;
type ResponseCallback = Box<dyn Fn(&Response) + Send + Sync>;
type HtmlCallback = Box<dyn Fn(&str, &str) + Send + Sync>; // (html, base_url)
type ErrorCallback = Box<dyn Fn(&dyn std::error::Error) + Send + Sync>;

/// 爬虫收集器 —— 框架核心
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

    /// 收到响应后的回调（传入 Response）
    on_response: Option<ResponseCallback>,

    /// HTML 解析后的回调（传入 html 内容和 base_url）
    on_html: Option<HtmlCallback>,

    /// 错误回调
    on_error: Option<ErrorCallback>,

    /// 已访问的 URL 集合（防重复）
    visited: Arc<Mutex<std::collections::HashSet<String>>>,

    /// 全局默认请求头
    default_headers: HashMap<String, String>,

    /// 是否跟踪链接（自动访问 HTML 中的链接）
    follow_links: bool,

    /// 链接选择器（用于跟踪链接时）
    link_selector: String,

    /// 最大并发数（0 = 无限）
    max_concurrency: usize,
}

impl Collector {
    /// 创建默认 Collector（使用 ReqwestClient）
    pub fn new() -> Self {
        Self {
            http_client: Arc::new(ReqwestClient::new()),
            on_request: None,
            on_response: None,
            on_html: None,
            on_error: None,
            visited: Arc::new(Mutex::new(std::collections::HashSet::new())),
            default_headers: HashMap::new(),
            follow_links: false,
            link_selector: "a[href]".into(),
            max_concurrency: 0,
        }
    }

    /// 使用自定义 HTTP 客户端构建 Collector
    pub fn with_client(client: impl HttpClient + 'static) -> Self {
        let mut c = Self::new();
        c.http_client = Arc::new(client);
        c
    }

    /// 设置全局默认请求头
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.default_headers.insert(key.to_string(), value.to_string());
    }

    /// 设置最大并发数
    pub fn set_max_concurrency(&mut self, n: usize) {
        self.max_concurrency = n;
    }

    /// 启用/禁用链接跟踪
    pub fn set_follow_links(&mut self, follow: bool) {
        self.follow_links = follow;
    }

    /// 自定义链接跟踪的选择器
    pub fn set_link_selector(&mut self, selector: &str) {
        self.link_selector = selector.to_string();
    }

    // ──────────────────────────────────────────────
    // 回调注册方法（链式调用）
    // ──────────────────────────────────────────────

    /// 注册请求前回调
    ///
    /// 在每次 HTTP 请求发送前调用，可用于：
    /// - 打印日志
    /// - 修改请求头
    /// - 添加 cookie
    pub fn on_request(&mut self, callback: impl Fn(&mut Request) + Send + Sync + 'static) {
        self.on_request = Some(Box::new(callback));
    }

    /// 注册响应回调
    ///
    /// 收到 HTTP 响应后调用，可用于：
    /// - 打印状态码
    /// - 记录日志
    pub fn on_response(&mut self, callback: impl Fn(&Response) + Send + Sync + 'static) {
        self.on_response = Some(Box::new(callback));
    }

    /// 注册 HTML 回调
    ///
    /// 解析到 HTML 内容后调用，可用于：
    /// - 提取链接
    /// - 提取文章内容
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

    /// 访问指定 URL（核心入口）
    ///
    /// 1. 构造 Request
    /// 2. 执行 on_request 回调
    /// 3. 调用 HTTP 客户端发送请求
    /// 4. 执行 on_response 回调
    /// 5. 如果是 HTML，执行 on_html 回调
    /// 6. 如果启用了 follow_links，自动递归访问链接
    pub async fn visit(&mut self, url: &str) -> Result<()> {
        let mut req = Request::get(url);
        // 合并默认请求头
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }

        self.do_request(&mut req).await
    }

    /// 执行请求（内部核心逻辑）
    async fn do_request(&mut self, req: &mut Request) -> Result<()> {
        // 检查是否已访问
        {
            let visited = self.visited.lock().unwrap();
            if visited.contains(&req.url) && !req.allow_revisit {
                return Ok(());
            }
        }

        // 执行 on_request 回调
        if let Some(ref cb) = self.on_request {
            cb(req);
        }

        // 发送请求
        let response = match self.http_client.get(&req.url, &req.headers).await {
            Ok(resp) => resp,
            Err(e) => {
                if let Some(ref cb) = self.on_error {
                    cb(&e);
                }
                return Err(e.into());
            }
        };

        // 标记已访问
        {
            let mut visited = self.visited.lock().unwrap();
            visited.insert(req.url.clone());
        }

        // 执行 on_response 回调
        if let Some(ref cb) = self.on_response {
            cb(&response);
        }

        // 如果是 HTML，执行 on_html 回调
        if response.is_html() {
            if let Some(ref cb) = self.on_html {
                cb(&response.body, &response.url);
            }

            // 如果启用了链接跟踪，提取并递归访问
            if self.follow_links {
                let links = extract_links(&response.body, &self.link_selector);
                let base_url = &response.url;

                // 解析相对链接为绝对链接
                let abs_links: Vec<String> = links
                    .iter()
                    .filter_map(|l| resolve_url(base_url, l))
                    .collect();

                for link in abs_links {
                    // 递归访问（每条链接创建新的 Request）
                    let mut child_req = Request::get(&link);
                    for (k, v) in &self.default_headers {
                        child_req.headers.insert(k.clone(), v.clone());
                    }
                    // 递归调用，Box::pin 避免栈溢出
                    Box::pin(self.do_request(&mut child_req)).await?;
                }
            }
        }

        Ok(())
    }

    // ──────────────────────────────────────────────
    // 便捷方法
    // ──────────────────────────────────────────────

    /// 一步到位：提取页面中所有链接
    ///
    /// 不触发回调，直接返回绝对 URL 列表
    pub async fn get_links(&self, url: &str, selector: &str) -> Result<Vec<String>> {
        let mut req = Request::get(url);
        for (k, v) in &self.default_headers {
            req.headers.insert(k.clone(), v.clone());
        }
        let response = self.http_client.get(&url, &req.headers).await?;
        let links = extract_links(&response.body, selector);
        let abs_links: Vec<String> = links
            .iter()
            .filter_map(|l| resolve_url(&response.url, l))
            .collect();
        Ok(abs_links)
    }

    /// 一步到位：提取文章内容
    ///
    /// 不触发回调，直接返回 Article 结构
    pub async fn get_article(&self, url: &str) -> Result<Article> {
        let response = self.http_client.get(url, &self.default_headers).await?;
        let article = extract_article(&response.body, &response.url);
        Ok(article)
    }

    /// 批量抓取文章（并发）
    pub async fn get_articles(&self, urls: &[String]) -> Vec<Result<Article>> {
        let mut handles = Vec::new();
        let client = self.http_client.clone();
        let headers = self.default_headers.clone();
        let concurrency = if self.max_concurrency == 0 {
            urls.len()
        } else {
            self.max_concurrency
        };

        // 使用 tokio 的信号量控制并发
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

        for url in urls {
            let url = url.clone();
            let client = client.clone();
            let headers = headers.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            handles.push(tokio::spawn(async move {
                let result = client.get(&url, &headers).await;
                drop(permit); // 释放信号量
                match result {
                    Ok(resp) => Ok(extract_article(&resp.body, &resp.url)),
                    Err(e) => Err(anyhow::anyhow!("请求失败: {}", e)),
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(r) => results.push(r),
                Err(e) => results.push(Err(anyhow::anyhow!("任务 panicked: {}", e))),
            }
        }
        results
    }

    /// 获取底层 HTTP 客户端引用
    pub fn client(&self) -> &dyn HttpClient {
        self.http_client.as_ref()
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
