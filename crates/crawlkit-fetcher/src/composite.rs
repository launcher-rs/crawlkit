//! 组合请求器模块
//!
//! 提供 CompositeFetcher，可以按优先级依次尝试多个 HttpClient，直到成功或全部失败。

use std::collections::HashMap;

use async_trait::async_trait;
use tracing::{info, warn};

use crawlkit_core::client::HttpClient;
use crawlkit_core::error::{CrawlError, Result};
use crawlkit_core::response::Response;

/// 组合请求器：依次尝试多个客户端，实现故障转移
///
/// 需要传入实现了 `HttpClient` trait 的客户端（如 `ReqwestClient`）。
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
}

impl CompositeFetcher {
    /// 创建组合请求器，按传入顺序依次尝试
    pub fn new(fetchers: Vec<Box<dyn HttpClient>>) -> Self {
        Self { fetchers }
    }

    /// 依次尝试各请求器直到成功
    pub async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let mut last_error = None;
        for fetcher in &self.fetchers {
            info!("尝试使用 {} 获取: {}", fetcher.name(), url);
            match fetcher.get(url, headers).await {
                Ok(response) => {
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
                    last_error = Some(e);
                }
            }
        }
        Err(CrawlError::AllFetchersFailed(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "无可用请求器".to_string()),
        ))
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
                    last_error = Some(e);
                }
            }
        }
        Err(CrawlError::AllFetchersFailed(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "无可用请求器".to_string()),
        ))
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

    fn name(&self) -> &str {
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

        fn name(&self) -> &str {
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
}
