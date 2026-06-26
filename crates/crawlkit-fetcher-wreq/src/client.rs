//! 基于 wreq 的 HTTP 客户端实现

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tracing::{debug, instrument, warn};

use wreq::Client;

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::response::Response;

/// 基于 wreq 的 HTTP 客户端
///
/// wreq 是 reqwest 的硬分叉，支持 TLS 指纹模拟（JA3/JA4/Akamai）、
/// 代理配置、指数退避重试等功能。
///
/// 可通过 `wreq_util` 设置浏览器指纹模拟（如 Chrome、Firefox、Safari 等）。
pub struct WreqClient {
    inner: Client,
    name: String,
    max_retries: usize,
}

impl WreqClient {
    /// 创建默认配置的客户端（构建失败时 panic）
    ///
    /// 仅适用于已知配置正确的场景。推荐使用 [`try_new`](Self::try_new)。
    pub fn new() -> Self {
        Self::try_new().expect("创建 wreq 客户端失败")
    }

    /// 创建默认配置的客户端（不 panic）
    pub fn try_new() -> Result<Self> {
        Self::builder().build()
    }

    /// 获取配置构建器
    pub fn builder() -> WreqClientBuilder {
        WreqClientBuilder::default()
    }

    /// 获取底层 wreq::Client 引用
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

impl Default for WreqClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for WreqClient {
    #[instrument(skip(self, headers), fields(name = %self.name))]
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        debug!(url, "开始 GET 请求");
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
            // 注意: wreq 的 `uri()` 返回原始请求 URI，非重定向后的最终 URL
            // 这是 wreq API 的限制，与 reqwest 的 `resp.url()` 行为不同
            let url = resp.uri().to_string();
            let body = resp.text().await?;
            Ok::<_, anyhow::Error>(Response {
                url,
                status,
                headers: response_headers,
                body,
            })
        };

        let result = fetch
            .retry(&ExponentialBuilder::default().with_max_times(max_retries))
            .await;
        match &result {
            Ok(resp) => debug!(status = resp.status, body_len = resp.body.len(), "GET 请求成功"),
            Err(e) => warn!(error = %e, "GET 请求失败（已重试 {max_retries} 次）"),
        }
        result.map_err(|e| CrawlError::Http(format!("wreq GET 请求失败(重试{max_retries}次): {e}")))
    }

    #[instrument(skip(self, headers, body), fields(name = %self.name))]
    async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response> {
        debug!(url, body_len = body.len(), "开始 POST 请求");
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
            // 注意: wreq 的 `uri()` 返回原始请求 URI，非重定向后的最终 URL
            let url = resp.uri().to_string();
            let body = resp.text().await?;
            Ok::<_, anyhow::Error>(Response {
                url,
                status,
                headers: response_headers,
                body,
            })
        };

        let result = fetch
            .retry(&ExponentialBuilder::default().with_max_times(max_retries))
            .await;
        match &result {
            Ok(resp) => debug!(status = resp.status, body_len = resp.body.len(), "POST 请求成功"),
            Err(e) => warn!(error = %e, "POST 请求失败（已重试 {max_retries} 次）"),
        }
        result.map_err(|e| CrawlError::Http(format!("wreq POST 请求失败(重试{max_retries}次): {e}")))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// WreqClient 构建器，支持链式配置
pub struct WreqClientBuilder {
    timeout: Option<Duration>,
    user_agent: Option<String>,
    name: String,
    max_retries: Option<usize>,
    proxy_url: Option<String>,
    proxy_user: Option<String>,
    proxy_pass: Option<String>,
    emulation: Option<wreq_util::Emulation>,
}

impl Default for WreqClientBuilder {
    fn default() -> Self {
        Self {
            timeout: None,
            user_agent: None,
            name: String::new(),
            max_retries: None,
            proxy_url: None,
            proxy_user: None,
            proxy_pass: None,
            emulation: None,
        }
    }
}

impl WreqClientBuilder {
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

    /// 设置浏览器指纹模拟
    ///
    /// 使用 `wreq_util` 提供的预定义浏览器配置（Chrome、Firefox、Safari 等），
    /// 可自定义 TLS 指纹/HTTP2/请求头。
    ///
    /// # 示例
    /// ```rust,ignore
    /// use wreq_util::{Emulation, Profile};
    ///
    /// WreqClient::builder()
    ///     .emulation(Emulation::builder()
    ///         .profile(Profile::Chrome120)
    ///         .build())
    ///     .build()?;
    /// ```
    pub fn emulation(mut self, emulation: wreq_util::Emulation) -> Self {
        self.emulation = Some(emulation);
        self
    }

    /// 构建 WreqClient
    pub fn build(self) -> Result<WreqClient> {
        let mut builder = Client::builder();

        let timeout = self.timeout.unwrap_or(Duration::from_secs(30));
        builder = builder.timeout(timeout);

        let user_agent = self
            .user_agent
            .unwrap_or_else(|| "crawlkit/0.2.0".to_string());
        builder = builder.user_agent(user_agent);

        if let Some(emu) = self.emulation {
            builder = builder.emulation(emu);
        }

        builder = builder.redirect(wreq::redirect::Policy::limited(10));
        builder = builder.pool_idle_timeout(Duration::from_secs(90));
        builder = builder.tcp_keepalive(Duration::from_secs(60));

        // 配置代理（优先使用构建器参数，其次使用环境变量）
        let proxy_url = self.proxy_url.or_else(|| env::var("PROXY_URL").ok());
        if let Some(ref url) = proxy_url {
            let proxy_user = self
                .proxy_user
                .or_else(|| env::var("PROXY_USER").ok())
                .unwrap_or_default();
            let proxy_pass = self
                .proxy_pass
                .or_else(|| env::var("PROXY_PASS").ok())
                .unwrap_or_default();

            // 根据 URL 协议选择代理类型
            let proxy = if url.starts_with("https") {
                wreq::Proxy::https(url)
            } else {
                wreq::Proxy::http(url)
            }
            .map_err(|e| CrawlError::Config(format!("代理配置失败: {e}")))?;

            let proxy = proxy.basic_auth(&proxy_user, &proxy_pass);
            builder = builder.proxy(proxy);
        }

        let inner = builder
            .build()
            .map_err(|e| CrawlError::Config(format!("构建 wreq 客户端失败: {e}")))?;
        let name = if self.name.is_empty() {
            "wreq".into()
        } else {
            self.name
        };
        let max_retries = self.max_retries.unwrap_or(3);

        Ok(WreqClient {
            inner,
            name,
            max_retries,
        })
    }
}
