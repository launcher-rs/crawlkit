//! 基于 reqwest 的 HTTP 客户端实现

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::time::Duration;

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::response::Response;

/// 默认 HTTP 客户端，基于 reqwest
///
/// 支持代理配置、指数退避重试、可配置超时等功能。
pub struct ReqwestClient {
    inner: Client,
    name: String,
    max_retries: usize,
}

impl ReqwestClient {
    /// 创建默认配置的客户端
    pub fn new() -> Self {
        Self::builder().build().expect("创建 reqwest 客户端失败")
    }

    /// 获取配置构建器
    pub fn builder() -> ReqwestClientBuilder {
        ReqwestClientBuilder::default()
    }

    /// 获取底层 reqwest::Client 引用
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
            .map_err(|e| {
                CrawlError::Http(format!("reqwest GET 请求失败(重试{max_retries}次): {e}"))
            })
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
        let body_owned = body;
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
            .map_err(|e| {
                CrawlError::Http(format!("reqwest POST 请求失败(重试{max_retries}次): {e}"))
            })
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
    /// 设置请求超时
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

        let timeout = self.timeout.unwrap_or(Duration::from_secs(30));
        builder = builder.timeout(timeout);

        let user_agent = self
            .user_agent
            .unwrap_or_else(|| "crawlkit/0.2.0".to_string());
        builder = builder.user_agent(user_agent);

        builder = builder.redirect(reqwest::redirect::Policy::limited(10));
        builder = builder.pool_idle_timeout(Duration::from_secs(90));
        builder = builder.tcp_keepalive(Duration::from_secs(60));

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
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| CrawlError::Config(format!("代理配置失败: {e}")))?
                .basic_auth(&proxy_user, &proxy_pass);
            builder = builder.proxy(proxy);
        }

        let inner = builder
            .build()
            .map_err(|e| CrawlError::Config(format!("构建 reqwest 客户端失败: {e}")))?;
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
