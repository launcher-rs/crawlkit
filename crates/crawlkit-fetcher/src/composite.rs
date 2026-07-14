//! 组合请求器模块
//!
//! 提供 CompositeFetcher，可以按优先级依次尝试多个 HttpClient，直到成功或全部失败。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::response::Response;

/// 后端失败回调类型
type BackendErrorCallback = Arc<dyn Fn(&str, &dyn std::error::Error) + Send + Sync>;

/// 组合请求器：依次尝试多个客户端，实现故障转移
///
/// 需要要传入实现了 `HttpClient` trait 的客户端（如 `ReqwestClient`）。
///
/// ```ignore
/// use crawlkit_fetcher::CompositeFetcher;
/// use crawlkit_fetcher_reqwest::ReqwestClient;
///
/// let fetcher = CompositeFetcher::new(vec![
///     Box::new(ReqwestClient::new()),
/// ]);
/// ```
pub struct CompositeFetcher {
    fetchers: Vec<Box<dyn HttpClient>>,
    detect_bot_challenge: bool,
    on_backend_error: Option<BackendErrorCallback>,
}

impl CompositeFetcher {
    /// 创建组合请求器，按传入顺序依次尝试
    pub fn new(fetchers: Vec<Box<dyn HttpClient>>) -> Self {
        Self {
            fetchers,
            detect_bot_challenge: true,
            on_backend_error: None,
        }
    }

    /// 启用/禁用机器人验证页面检测（默认开启）
    ///
    /// 启用时，如果某个后端返回 HTTP 200 但内容为机器人验证页面（如 PerimeterX、Cloudflare），
    /// 会视为该后端失败，自动尝试下一个后端。
    pub fn detect_bot_challenge(mut self, enabled: bool) -> Self {
        self.detect_bot_challenge = enabled;
        self
    }

    /// 注册后端失败回调
    ///
    /// 每个后端请求失败（网络错误或机器人验证）时触发。
    /// 参数：`(backend_name, error)`。
    pub fn on_backend_error(
        mut self,
        callback: impl Fn(&str, &dyn std::error::Error) + Send + Sync + 'static,
    ) -> Self {
        self.on_backend_error = Some(Arc::new(callback));
        self
    }

    /// 依次尝试各请求器直到成功
    pub async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let mut last_error = None;
        for fetcher in &self.fetchers {
            info!("尝试使用 {} 获取: {}", fetcher.name(), url);
            match fetcher.get(url, headers).await {
                Ok(response) => {
                    // 检测机器人验证页面，视为失败并尝试下一个后端
                    if self.detect_bot_challenge && response.is_bot_challenge() {
                        let err = CrawlError::BotChallenge(format!(
                            "{}: {}",
                            fetcher.name(),
                            response.url
                        ));
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

    /// 依次尝试各请求器发送 POST
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

    /// 获取请求器数量
    pub fn len(&self) -> usize {
        self.fetchers.len()
    }

    /// 是否为空
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[tokio::test]
    async fn test_composite_fetcher_first_success() {
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
    async fn test_composite_fetcher_fallback() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::fail("m1", "失败")),
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
    async fn test_composite_fetcher_all_fail() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(MockClient::fail("m1", "错误1")),
            Box::new(MockClient::fail("m2", "错误2")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_composite_fetcher_empty() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
    }

    /// 模拟返回机器人验证页面的客户端
    struct BotChallengeClient {
        name: &'static str,
    }

    #[async_trait]
    impl HttpClient for BotChallengeClient {
        async fn get(&self, _url: &str, _headers: &HashMap<String, String>) -> Result<Response> {
            Ok(Response {
                url: "http://test.com".to_string(),
                status: 200,
                headers: HashMap::from([(
                    "content-type".into(),
                    "text/html".into(),
                )]),
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

    #[tokio::test]
    async fn test_bot_challenge_fallback_to_next_backend() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "primary" }),
            Box::new(MockClient::success("fallback", "real content")),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite
            .get("http://test.com", &HashMap::new())
            .await
            .unwrap();
        // 第一个后端返回机器人验证页面，自动跳到第二个后端
        assert_eq!(result.body, "real content");
    }

    #[tokio::test]
    async fn test_bot_challenge_detection_disabled() {
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
    async fn test_bot_challenge_all_backends_blocked() {
        let fetchers: Vec<Box<dyn HttpClient>> = vec![
            Box::new(BotChallengeClient { name: "m1" }),
            Box::new(BotChallengeClient { name: "m2" }),
        ];
        let composite = CompositeFetcher::new(fetchers);

        let result = composite.get("http://test.com", &HashMap::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_on_backend_error_fires_per_backend() {
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
}
