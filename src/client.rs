//! HTTP 客户端抽象层
//!
//! 核心 trait `HttpClient` 定义了所有 HTTP 客户端必须实现的接口。
//! 默认提供 `ReqwestClient` 实现，支持代理配置和重试机制。
//! 用户可自行替换为 wreq、ureq 等。

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::time::Duration;

use crate::error::Result;
use crate::response::Response;

/// HTTP 客户端 trait —— 所有请求后端的统一接口
///
/// 实现此 trait 即可接入框架，例如：
/// - `ReqwestClient`（默认）
/// - `WreqClient`（wreq 封装）
/// - 自定义 mock 实现用于测试
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// 发送 GET 请求并返回统一的 Response
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response>;

    /// 发送 POST 请求
    async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response>;

    /// 返回客户端名称，用于日志/调试
    fn name(&self) -> &str;
}

/// 默认 HTTP 客户端，基于 reqwest
///
/// 支持以下功能：
/// - 代理配置（通过环境变量 PROXY_URL, PROXY_USER, PROXY_PASS）
/// - 指数退避重试
/// - 可配置超时时间
pub struct ReqwestClient {
    inner: Client,
    /// 客户端标识名称
    name: String,
    /// 最大重试次数
    max_retries: usize,
}

impl ReqwestClient {
    /// 创建默认配置的 ReqwestClient
    pub fn new() -> Self {
        Self::builder().build().expect("Failed to create reqwest client")
    }

    /// 自定义配置构建
    pub fn builder() -> ReqwestClientBuilder {
        ReqwestClientBuilder::default()
    }

    /// 获取底层 reqwest::Client（用于高级用法）
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let client = self.inner.clone();
        let url_owned = url.to_owned();
        let headers_owned = headers.clone();
        let max_retries = self.max_retries;

        let fetch = || async {
            let mut req = client.get(&url_owned);
            for (k, v) in &headers_owned {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req.send().await?;
            let status = resp.status().as_u16();
            let response_headers: HashMap<String, String> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let url = resp.url().clone();
            let body = resp.text().await?;
            Ok::<_, anyhow::Error>(Response {
                url: url.to_string(),
                status,
                headers: response_headers,
                body,
            })
        };

        fetch
            .retry(&ExponentialBuilder::default().with_max_times(max_retries))
            .await
            .map_err(|e| anyhow::anyhow!("reqwest 请求失败(重试{}次): {}", max_retries, e).into())
    }

    async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response> {
        let client = self.inner.clone();
        let url_owned = url.to_owned();
        let headers_owned = headers.clone();
        let body_owned = body.clone();
        let max_retries = self.max_retries;

        let fetch = || async {
            let mut req = client.post(&url_owned).body(body_owned.clone());
            for (k, v) in &headers_owned {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req.send().await?;
            let status = resp.status().as_u16();
            let response_headers: HashMap<String, String> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let url = resp.url().clone();
            let body = resp.text().await?;
            Ok::<_, anyhow::Error>(Response {
                url: url.to_string(),
                status,
                headers: response_headers,
                body,
            })
        };

        fetch
            .retry(&ExponentialBuilder::default().with_max_times(max_retries))
            .await
            .map_err(|e| anyhow::anyhow!("reqwest 请求失败(重试{}次): {}", max_retries, e).into())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// ReqwestClient 构建器，支持链式配置
#[derive(Default)]
pub struct ReqwestClientBuilder {
    timeout: Option<Duration>,
    user_agent: Option<String>,
    name: String,
    max_retries: Option<usize>,
    proxy_url: Option<String>,
    proxy_user: Option<String>,
    proxy_pass: Option<String>,
}

impl ReqwestClientBuilder {
    /// 设置请求超时时间
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    /// 设置 User-Agent
    pub fn user_agent(mut self, ua: &str) -> Self {
        self.user_agent = Some(ua.to_string());
        self
    }

    /// 设置客户端名称
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// 设置最大重试次数
    pub fn max_retries(mut self, n: usize) -> Self {
        self.max_retries = Some(n);
        self
    }

    /// 设置代理 URL
    pub fn proxy_url(mut self, url: &str) -> Self {
        self.proxy_url = Some(url.to_string());
        self
    }

    /// 设置代理用户名
    pub fn proxy_user(mut self, user: &str) -> Self {
        self.proxy_user = Some(user.to_string());
        self
    }

    /// 设置代理密码
    pub fn proxy_pass(mut self, pass: &str) -> Self {
        self.proxy_pass = Some(pass.to_string());
        self
    }

    /// 构建 ReqwestClient
    pub fn build(self) -> Result<ReqwestClient> {
        let mut builder = Client::builder();

        // 设置超时时间
        let timeout = self.timeout.unwrap_or(Duration::from_secs(30));
        builder = builder.timeout(timeout);

        // 设置 User-Agent
        let user_agent = self
            .user_agent
            .unwrap_or_else(|| "crawlkit/0.1.0".to_string());
        builder = builder.user_agent(user_agent);

        // 配置重定向策略
        builder = builder.redirect(reqwest::redirect::Policy::limited(10));

        // 配置连接池
        builder = builder.pool_idle_timeout(Duration::from_secs(90));
        builder = builder.tcp_keepalive(Duration::from_secs(60));

        // 配置代理（优先使用构建器参数，其次使用环境变量）
        let proxy_url = self.proxy_url.or_else(|| env::var("PROXY_URL").ok());
        if let Some(proxy_url) = proxy_url {
            let proxy_user = self
                .proxy_user
                .or_else(|| env::var("PROXY_USER").ok())
                .unwrap_or_default();
            let proxy_pass = self
                .proxy_pass
                .or_else(|| env::var("PROXY_PASS").ok())
                .unwrap_or_default();
            let proxy = reqwest::Proxy::all(&proxy_url)?
                .basic_auth(&proxy_user, &proxy_pass);
            builder = builder.proxy(proxy);
        }

        let inner = builder.build()?;
        let name = if self.name.is_empty() {
            "reqwest".into()
        } else {
            self.name
        };
        let max_retries = self.max_retries.unwrap_or(3);

        Ok(ReqwestClient {
            inner,
            name,
            max_retries,
        })
    }
}
