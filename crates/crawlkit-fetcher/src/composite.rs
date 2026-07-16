//! 组合请求器模块
//!
//! 提供 [`CompositeFetcher`]，按优先级依次尝试多个 [`HttpClient`]，实现故障转移。
//!
//! # 核心能力
//!
//! - **自动故障转移**：当前端返回网络错误时，自动切换到下一个后端
//! - **拦截检测**：自动识别机器人验证页面（Bot Challenge）和访问被拒绝页面（Access Denied），
//!   视为该后端失败，触发故障转移
//! - **自定义检测**：通过 `on_detect_bot_challenge` / `on_detect_access_denied` 回调
//!   注入业务特定的拦截判断逻辑
//!
//! # 检测流程
//!
//! 每个后端响应后，按以下顺序检测：
//!
//! 1. 内置 `is_bot_challenge()` 检测（PerimeterX、Cloudflare 等）
//! 2. 内置 `is_access_denied()` 检测（Akamai、Cloudflare 403 等）
//! 3. 自定义 `on_detect_bot_challenge` 回调
//! 4. 自定义 `on_detect_access_denied` 回调
//!
//! 任一命中即视为该后端失败，继续尝试下一个。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::response::Response;

/// 后端失败回调类型：`(backend_name, error)`
type BackendErrorCallback = Arc<dyn Fn(&str, &dyn std::error::Error) + Send + Sync>;

/// 自定义检测回调类型：返回 `true` 表示检测到拦截
type DetectCallback = Arc<dyn Fn(&Response) -> bool + Send + Sync>;

/// 组合请求器：依次尝试多个客户端，实现故障转移
///
/// 需要传入实现了 [`HttpClient`] trait 的客户端（如 `ReqwestClient`、`WreqClient`）。
///
/// # 基本用法
///
/// ```ignore
/// use crawlkit_fetcher::CompositeFetcher;
/// use crawlkit_fetcher_reqwest::ReqwestClient;
///
/// let fetcher = CompositeFetcher::new(vec![
///     Box::new(ReqwestClient::builder().name("primary").build().unwrap()),
///     Box::new(ReqwestClient::builder().name("fallback").build().unwrap()),
/// ]);
///
/// let resp = fetcher.get("https://example.com", &HashMap::new()).await?;
/// ```
///
/// # 自定义拦截检测
///
/// ```ignore
/// let fetcher = CompositeFetcher::new(backends)
///     // 自定义机器人验证检测
///     .on_detect_bot_challenge(|resp| {
///         resp.body.contains("custom-captcha-widget")
///     })
///     // 自定义访问被拒绝检测
///     .on_detect_access_denied(|resp| {
///         resp.status == 403 && resp.body.contains("blocked by security")
///     });
/// ```
pub struct CompositeFetcher {
    /// 后端列表（按优先级排序）
    fetchers: Vec<Box<dyn HttpClient>>,
    /// 是否启用内置机器人验证页面检测（默认开启）
    detect_bot_challenge: bool,
    /// 是否启用内置访问被拒绝检测（默认开启）
    detect_access_denied: bool,
    /// 后端失败回调（网络错误、拦截检测均触发）
    on_backend_error: Option<BackendErrorCallback>,
    /// 自定义机器人验证检测回调（与内置检测叠加）
    on_detect_bot_challenge: Option<DetectCallback>,
    /// 自定义访问被拒绝检测回调（与内置检测叠加）
    on_detect_access_denied: Option<DetectCallback>,
}

impl CompositeFetcher {
    /// 创建组合请求器，按传入顺序依次尝试
    ///
    /// 默认启用内置的 `is_bot_challenge()` 和 `is_access_denied()` 检测。
    pub fn new(fetchers: Vec<Box<dyn HttpClient>>) -> Self {
        Self {
            fetchers,
            detect_bot_challenge: true,
            detect_access_denied: true,
            on_backend_error: None,
            on_detect_bot_challenge: None,
            on_detect_access_denied: None,
        }
    }

    /// 启用/禁用机器人验证页面检测（默认开启）
    ///
    /// 启用时，如果某个后端返回 HTTP 200 但内容为机器人验证页面
    /// （如 PerimeterX、Cloudflare challenge、DataDome），
    /// 会视为该后端失败（[`CrawlError::BotChallenge`]），自动尝试下一个后端。
    ///
    /// 关闭后跳过内置检测，直接当作正常响应处理。
    /// 仍会执行自定义 `on_detect_bot_challenge` 回调（如有注册）。
    pub fn detect_bot_challenge(mut self, enabled: bool) -> Self {
        self.detect_bot_challenge = enabled;
        self
    }

    /// 启用/禁用访问被拒绝检测（默认开启）
    ///
    /// 启用时，如果某个后端返回 HTTP 403/401 等拒绝访问响应
    /// （如 Akamai CDN、Cloudflare WAF、AWS WAF），
    /// 会视为该后端失败（[`CrawlError::AccessDenied`]），自动尝试下一个后端。
    ///
    /// 关闭后跳过内置检测，直接当作正常响应处理。
    /// 仍会执行自定义 `on_detect_access_denied` 回调（如有注册）。
    pub fn detect_access_denied(mut self, enabled: bool) -> Self {
        self.detect_access_denied = enabled;
        self
    }

    /// 自定义机器人验证页面检测逻辑
    ///
    /// 回调接收 `&Response`，返回 `true` 表示该响应为机器人验证页面。
    ///
    /// 与内置 `is_bot_challenge()` **叠加**生效：
    /// 内置检测已覆盖主流反爬服务，此回调用于处理内置未覆盖的特殊场景。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let fetcher = CompositeFetcher::new(backends)
    ///     .on_detect_bot_challenge(|resp| {
    ///         // 检测自定义验证页面
    ///         resp.body.contains("please complete the verification")
    ///     });
    /// ```
    pub fn on_detect_bot_challenge(
        mut self,
        callback: impl Fn(&Response) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.on_detect_bot_challenge = Some(Arc::new(callback));
        self
    }

    /// 自定义访问被拒绝检测逻辑
    ///
    /// 回调接收 `&Response`，返回 `true` 表示该响应为访问被拒绝页面。
    ///
    /// 与内置 `is_access_denied()` **叠加**生效：
    /// 内置检测已覆盖 Akamai/Cloudflare 等常见 CDN，此回调用于处理
    /// 企业内部 WAF、自定义拦截页面等场景。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let fetcher = CompositeFetcher::new(backends)
    ///     .on_detect_access_denied(|resp| {
    ///         resp.status == 403 && resp.body.contains("corporate security block")
    ///     });
    /// ```
    pub fn on_detect_access_denied(
        mut self,
        callback: impl Fn(&Response) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.on_detect_access_denied = Some(Arc::new(callback));
        self
    }

    /// 注册后端失败回调
    ///
    /// 每个后端请求失败时触发，失败原因包括：
    /// - 网络错误（超时、连接失败等）
    /// - 拦截检测命中（BotChallenge / AccessDenied）
    ///
    /// 参数：`(backend_name, error)`。
    pub fn on_backend_error(
        mut self,
        callback: impl Fn(&str, &dyn std::error::Error) + Send + Sync + 'static,
    ) -> Self {
        self.on_backend_error = Some(Arc::new(callback));
        self
    }

    /// 依次尝试各请求器 GET 直到成功
    ///
    /// 按优先级遍历后端列表，对每个后端：
    /// 1. 发送 GET 请求
    /// 2. 检测拦截（内置 + 自定义）
    /// 3. 命中则记录错误并尝试下一个后端
    /// 4. 未命中则返回成功响应
    ///
    /// 所有后端均失败时返回 [`CrawlError::AllFetchersFailed`]。
    pub async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let mut last_error = None;
        for fetcher in &self.fetchers {
            info!("尝试使用 {} 获取: {}", fetcher.name(), url);
            match fetcher.get(url, headers).await {
                Ok(response) => {
                    // ── 拦截检测（按优先级依次检查） ──

                    // 1. 内置机器人验证检测
                    if self.detect_bot_challenge && response.is_bot_challenge() {
                        let err =
                            CrawlError::BotChallenge(format!("{}: {}", fetcher.name(), response.url));
                        warn!(
                            "{} 获取到机器人验证页面: {} ({} bytes)，尝试下一个后端",
                            fetcher.name(),
                            url,
                            response.body.len()
                        );
                        if let Some(ref cb) = self.on_backend_error {
                            cb(fetcher.name(), &err);
                        }
                        last_error = Some(err);
                        continue;
                    }

                    // 2. 内置访问被拒绝检测
                    if self.detect_access_denied && response.is_access_denied() {
                        let err = CrawlError::AccessDenied(format!(
                            "{}: {}",
                            fetcher.name(),
                            response.url
                        ));
                        warn!(
                            "{} 获取到访问被拒绝页面: {} ({} bytes)，尝试下一个后端",
                            fetcher.name(),
                            url,
                            response.body.len()
                        );
                        if let Some(ref cb) = self.on_backend_error {
                            cb(fetcher.name(), &err);
                        }
                        last_error = Some(err);
                        continue;
                    }

                    // 3. 自定义机器人验证检测
                    if let Some(ref cb) = self.on_detect_bot_challenge {
                        if cb(&response) {
                            let err = CrawlError::BotChallenge(format!(
                                "{} (自定义检测): {}",
                                fetcher.name(),
                                response.url
                            ));
                            warn!(
                                "{} 自定义检测到机器人验证页面: {}，尝试下一个后端",
                                fetcher.name(),
                                url,
                            );
                            if let Some(ref err_cb) = self.on_backend_error {
                                err_cb(fetcher.name(), &err);
                            }
                            last_error = Some(err);
                            continue;
                        }
                    }

                    // 4. 自定义访问被拒绝检测
                    if let Some(ref cb) = self.on_detect_access_denied {
                        if cb(&response) {
                            let err = CrawlError::AccessDenied(format!(
                                "{} (自定义检测): {}",
                                fetcher.name(),
                                response.url
                            ));
                            warn!(
                                "{} 自定义检测到访问被拒绝页面: {}，尝试下一个后端",
                                fetcher.name(),
                                url,
                            );
                            if let Some(ref err_cb) = self.on_backend_error {
                                err_cb(fetcher.name(), &err);
                            }
                            last_error = Some(err);
                            continue;
                        }
                    }

                    // ── 所有检测通过，返回成功 ──
                    info!(
                        "{} 获取成功: {} ({} bytes)",
                        fetcher.name(),
                        url,
                        response.body.len()
                    );
                    return Ok(response);
                }
                Err(e) => {
                    warn!("{} 获取失败: {}", fetcher.name(), e);
                    if let Some(ref cb) = self.on_backend_error {
                        cb(fetcher.name(), &e);
                    }
                    last_error = Some(e);
                }
            }
        }
        Err(CrawlError::AllFetchersFailed(last_error.map_or_else(
            || "无可用请求器".to_string(),
            |e| e.to_string(),
        )))
    }

    /// 依次尝试各请求器发送 POST 直到成功
    ///
    /// 检测逻辑与 [`get()`](Self::get) 完全一致。
    pub async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response> {
        let mut last_error = None;
        for fetcher in &self.fetchers {
            info!("尝试使用 {} 发送 POST: {}", fetcher.name(), url);
            match fetcher.post(url, headers, body.clone()).await {
                Ok(response) => {
                    // ── 拦截检测（与 get() 一致） ──

                    // 1. 内置机器人验证检测
                    if self.detect_bot_challenge && response.is_bot_challenge() {
                        let err = CrawlError::BotChallenge(format!(
                            "{}: {}",
                            fetcher.name(),
                            response.url
                        ));
                        warn!(
                            "{} POST 获取到机器人验证页面: {} ({} bytes)，尝试下一个后端",
                            fetcher.name(),
                            url,
                            response.body.len()
                        );
                        if let Some(ref cb) = self.on_backend_error {
                            cb(fetcher.name(), &err);
                        }
                        last_error = Some(err);
                        continue;
                    }

                    // 2. 内置访问被拒绝检测
                    if self.detect_access_denied && response.is_access_denied() {
                        let err = CrawlError::AccessDenied(format!(
                            "{}: {}",
                            fetcher.name(),
                            response.url
                        ));
                        warn!(
                            "{} POST 获取到访问被拒绝页面: {} ({} bytes)，尝试下一个后端",
                            fetcher.name(),
                            url,
                            response.body.len()
                        );
                        if let Some(ref cb) = self.on_backend_error {
                            cb(fetcher.name(), &err);
                        }
                        last_error = Some(err);
                        continue;
                    }

                    // 3. 自定义机器人验证检测
                    if let Some(ref cb) = self.on_detect_bot_challenge {
                        if cb(&response) {
                            let err = CrawlError::BotChallenge(format!(
                                "{} (自定义检测): {}",
                                fetcher.name(),
                                response.url
                            ));
                            warn!(
                                "{} POST 自定义检测到机器人验证页面: {}，尝试下一个后端",
                                fetcher.name(),
                                url,
                            );
                            if let Some(ref err_cb) = self.on_backend_error {
                                err_cb(fetcher.name(), &err);
                            }
                            last_error = Some(err);
                            continue;
                        }
                    }

                    // 4. 自定义访问被拒绝检测
                    if let Some(ref cb) = self.on_detect_access_denied {
                        if cb(&response) {
                            let err = CrawlError::AccessDenied(format!(
                                "{} (自定义检测): {}",
                                fetcher.name(),
                                response.url
                            ));
                            warn!(
                                "{} POST 自定义检测到访问被拒绝页面: {}，尝试下一个后端",
                                fetcher.name(),
                                url,
                            );
                            if let Some(ref err_cb) = self.on_backend_error {
                                err_cb(fetcher.name(), &err);
                            }
                            last_error = Some(err);
                            continue;
                        }
                    }

                    // ── 所有检测通过，返回成功 ──
                    info!(
                        "{} POST 成功: {} ({} bytes)",
                        fetcher.name(),
                        url,
                        response.body.len()
                    );
                    return Ok(response);
                }
                Err(e) => {
                    warn!("{} POST 失败: {}", fetcher.name(), e);
                    if let Some(ref cb) = self.on_backend_error {
                        cb(fetcher.name(), &e);
                    }
                    last_error = Some(e);
                }
            }
        }
        Err(CrawlError::AllFetchersFailed(last_error.map_or_else(
            || "无可用请求器".to_string(),
            |e| e.to_string(),
        )))
    }

    /// 获取后端数量
    pub fn len(&self) -> usize {
        self.fetchers.len()
    }

    /// 是否没有后端
    pub fn is_empty(&self) -> bool {
        self.fetchers.is_empty()
    }
}

#[async_trait]
impl HttpClient for CompositeFetcher {
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        CompositeFetcher::get(self, url, headers).await
    }

    async fn post(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<Response> {
        CompositeFetcher::post(self, url, headers, body).await
    }

    fn name(&self) -> &'static str {
        "composite"
    }
}

// ─────────────────────────────────────────────────────────────
// 单元测试
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ════════════════════════════════════════
    //  Mock 客户端
    // ════════════════════════════════════════

    /// 模拟成功响应的客户端
    struct MockClient {
        name: &'static str,
        response: Option<String>,
        error: Option<String>,
        call_count: AtomicUsize,
    }

    impl MockClient {
        fn success(name: &'static str, content: &str) -> Self {
            Self {
                name,
                response: Some(content.to_string()),
                error: None,
                call_count: AtomicUsize::new(0),
            }
        }

        fn fail(name: &'static str, error: &str) -> Self {
            Self {
                name,
                response: None,
                error: Some(error.to_string()),
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockClient {
        async fn get(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<Response> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            if let Some(body) = &self.response {
                Ok(Response {
                    url: "http://test.com".to_string(),
                    status: 200,
                    headers: HashMap::new(),
                    body: body.clone(),
                })
            } else {
                Err(CrawlError::Http(
                    self.error.as_deref().unwrap_or("模拟错误").to_string(),
                ))
            }
        }

        async fn post(
            &self,
            _url: &str,
            _headers: &HashMap<String, String>,
            _body: Vec<u8>,
        ) -> Result<Response> {
            self.get(_url, _headers).await
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    /// 模拟返回机器人验证页面的客户端（HTTP 200 + PerimeterX 特征）
    struct BotChallengeClient {
        name: &'static str,
    }

    #[async_trait]
    impl HttpClient for BotChallengeClient {
        async fn get(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<Response> {
            Ok(Response {
                url: "http://test.com".to_string(),
                status: 200,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: r#"<!DOCTYPE html>
<html lang="en">
<head><title>Access to this page has been denied</title></head>
<body>
<script>
window._pxUuid = 'f0fb4dce-7f56-11f1-8179-f7818979d289';
window._pxAppId = 'PX6zcfGH4h';
</script>
</body>
</html>"#
                    .to_string(),
            })
        }

        async fn post(
            &self,
            url: &str,
            headers: &HashMap<String, String>,
            _body: Vec<u8>,
        ) -> Result<Response> {
            self.get(url, headers).await
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    /// 模拟返回403访问被拒绝页面的客户端（Akamai CDN 格式）
    struct AccessDeniedClient {
        name: &'static str,
    }

    #[async_trait]
    impl HttpClient for AccessDeniedClient {
        async fn get(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<Response> {
            Ok(Response {
                url: "http://test.com".to_string(),
                status: 403,
                headers: HashMap::from([("content-type".into(), "text/html".into())]),
                body: r#"<HTML><HEAD>
<TITLE>Access Denied</TITLE>
</HEAD><BODY>
<H1>Access Denied</H1>
You don't have permission to access this resource.
Reference #123.456.789
https://errors.edgesuite.net/123.456.789
</BODY>
</HTML>"#
                    .to_string(),
            })
        }

        async fn post(
            &self,
            url: &str,
            headers: &HashMap<String, String>,
            _body: Vec<u8>,
        ) -> Result<Response> {
            self.get(url, headers).await
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    // ════════════════════════════════════════
    //  基本故障转移测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn first_success() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::success("m1", "content from m1")),
            Box::new(MockClient::success("m2", "content from m2")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "content from m1");
    }

    #[tokio::test]
    async fn fallback_on_network_error() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::fail("m1", "网络错误")),
            Box::new(MockClient::success("m2", "content from m2")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "content from m2");
    }

    #[tokio::test]
    async fn all_fail_returns_error() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::fail("m1", "错误1")),
            Box::new(MockClient::fail("m2", "错误2")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CrawlError::AllFetchersFailed(msg) => assert!(msg.contains("错误2")),
            other => panic!("期望 AllFetchersFailed，实际: {:?}", other),
        }
    }

    #[tokio::test]
    async fn empty_fetchers_returns_error() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
    }

    // ════════════════════════════════════════
    //  Bot Challenge 检测测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn bot_challenge_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "real content");
    }

    #[tokio::test]
    async fn bot_challenge_detection_disabled() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers).detect_bot_challenge(false);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        // 关闭检测后，直接返回机器人验证页面
        assert!(result.body.contains("_pxUuid"));
    }

    #[tokio::test]
    async fn bot_challenge_all_backends_blocked() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "m1" }),
            Box::new(BotChallengeClient { name: "m2" }),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
    }

    // ════════════════════════════════════════
    //  Access Denied 检测测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn access_denied_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(AccessDeniedClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "real content");
    }

    #[tokio::test]
    async fn access_denied_detection_disabled() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(AccessDeniedClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers).detect_access_denied(false);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        // 关闭检测后，直接返回403拒绝页面
        assert!(result.body.contains("Access Denied"));
        assert_eq!(result.status, 403);
    }

    // ════════════════════════════════════════
    //  自定义检测回调测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn custom_bot_challenge_callback_triggers_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::success(
                "primary",
                "<html><body>Please complete the verification</body></html>",
            )),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers).on_detect_bot_challenge(|resp| {
            let lower = resp.body.to_ascii_lowercase();
            lower.contains("please complete the verification")
        });

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "real content");
    }

    #[tokio::test]
    async fn custom_access_denied_callback_triggers_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::success(
                "primary",
                "<html><body>blocked by corporate security policy</body></html>",
            )),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers).on_detect_access_denied(|resp| {
            resp.body.contains("blocked by") && resp.body.contains("security")
        });

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(result.body, "real content");
    }

    #[tokio::test]
    async fn custom_callbacks_not_triggered_when_normal_content() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![Box::new(MockClient::success(
            "m1",
            "<html><body>Normal page content</body></html>",
        ))];
        let composite = CompositeFetcher::new(fetchers)
            .on_detect_bot_challenge(|resp| resp.body.contains("captcha"))
            .on_detect_access_denied(|resp| resp.body.contains("blocked"));

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        assert_eq!(
            result.body,
            "<html><body>Normal page content</body></html>"
        );
    }

    #[tokio::test]
    async fn custom_callback_error_type_is_correct() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::success("primary", "<html><body>captcha here</body></html>")),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers).on_detect_bot_challenge(|resp| {
            resp.body.contains("captcha")
        });

        let result = composite.get("http://test.com", &HashMap::new()).await;
        // 第一个后端被自定义回调拦截，第二个成功
        assert!(result.is_ok());
        assert_eq!(result.unwrap().body, "real content");
    }

    // ════════════════════════════════════════
    //  POST 方法测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn post_bot_challenge_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "post content")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .post("http://test.com", &HashMap::new(), vec![])
            .await
            .unwrap();
        assert_eq!(result.body, "post content");
    }

    #[tokio::test]
    async fn post_access_denied_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(AccessDeniedClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "post content")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .post("http://test.com", &HashMap::new(), vec![])
            .await
            .unwrap();
        assert_eq!(result.body, "post content");
    }

    // ════════════════════════════════════════
    //  回调触发测试
    // ════════════════════════════════════════

    #[tokio::test]
    async fn on_backend_error_fires_per_failed_backend() {
        let error_count = Arc::new(AtomicUsize::new(0));
        let count = error_count.clone();

        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "primary" }),
            Box::new(BotChallengeClient { name: "fallback" }),
        ];
        let composite = CompositeFetcher::new(fetchers).on_backend_error(move |_name, _err| {
            count.fetch_add(1, Ordering::Relaxed);
        });

        let _ = composite.get("http://test.com", &HashMap::new()).await;
        assert_eq!(error_count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn on_backend_error_fires_for_access_denied() {
        let error_count = Arc::new(AtomicUsize::new(0));
        let count = error_count.clone();

        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(AccessDeniedClient { name: "primary" }),
            Box::new(AccessDeniedClient { name: "fallback" }),
        ];
        let composite = CompositeFetcher::new(fetchers).on_backend_error(move |_name, _err| {
            count.fetch_add(1, Ordering::Relaxed);
        });

        let _ = composite.get("http://test.com", &HashMap::new()).await;
        assert_eq!(error_count.load(Ordering::Relaxed), 2);
    }

    // ════════════════════════════════════════
    //  辅助方法测试
    // ════════════════════════════════════════

    #[test]
    fn len_and_is_empty() {
        let empty: Vec<Box<dyn HttpClient>> = vec![];
        assert!(CompositeFetcher::new(empty).is_empty());

        let fetchers: Vec<Box<dyn HttpClient>> = vec![Box::new(MockClient::success("m1", ""))];
        let c = CompositeFetcher::new(fetchers);
        assert_eq!(c.len(), 1);
        assert!(!c.is_empty());
    }
}
