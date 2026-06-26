//! 自定义 Jina Reader 客户端示例
//!
//! 运行：
//! `cargo run -p crawlkit --example custom_jina_client`
//!
//! 可选环境变量：
//! - `JINA_API_TOKEN`：Jina Reader API Token

use std::{collections::HashMap, env};

use async_trait::async_trait;
use crawlkit::{Collector, CrawlError, HttpClient, Response, Result};

/// 用户侧自定义 Jina Reader 客户端。
///
/// 这种外部服务后端无需放进 crawlkit 官方 crate，只要实现 `HttpClient`
/// 就能通过 `Collector::with_client` 或 `CompositeFetcher` 接入。
pub struct JinaClient {
    client: reqwest::Client,
    token: Option<String>,
}

impl JinaClient {
    /// 创建 Jina Reader 客户端。
    pub fn new(token: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
        }
    }
}

#[async_trait]
impl HttpClient for JinaClient {
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let reader_url = format!("https://r.jina.ai/{url}");
        let mut req = self
            .client
            .get(&reader_url)
            .header("Accept", "text/markdown");

        for (key, value) in headers {
            req = req.header(key, value);
        }

        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CrawlError::Http(format!("Jina 请求失败: {e}")))?;

        response_from_reqwest(resp).await
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
        "jina"
    }
}

async fn response_from_reqwest(resp: reqwest::Response) -> Result<Response> {
    let status = resp.status().as_u16();
    let url = resp.url().to_string();
    let headers = resp
        .headers()
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp
        .text()
        .await
        .map_err(|e| CrawlError::Http(format!("读取 Jina 响应失败: {e}")))?;

    Ok(Response {
        url,
        status,
        headers,
        body,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = env::var("JINA_API_TOKEN").ok();
    let client = JinaClient::new(token);
    let collector = Collector::with_client(client);

    let response = collector
        .client()
        .get("https://example.com", &HashMap::new())
        .await?;

    println!("客户端: {}", collector.client().name());
    println!("状态码: {}", response.status);
    println!(
        "内容预览:\n{}",
        response.body.chars().take(500).collect::<String>()
    );

    Ok(())
}
