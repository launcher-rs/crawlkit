//! 自定义 Firecrawl 客户端示例
//!
//! 运行：
//! `FIRECRAWL_API_KEY=fc-xxx cargo run -p crawlkit --example custom_firecrawl_client`
//!
//! 可选环境变量：
//! - `FIRECRAWL_API_KEY`：Firecrawl API Key
//! - `FIRECRAWL_SCRAPE_ENDPOINT`：自定义 scrape 端点，默认 `https://api.firecrawl.dev/v2/scrape`

use std::{collections::HashMap, env};

use async_trait::async_trait;
use crawlkit::{Collector, CrawlError, HttpClient, Response, Result};
use serde_json::{Value, json};

/// 用户侧自定义 Firecrawl 客户端。
pub struct FirecrawlClient {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl FirecrawlClient {
    /// 创建 Firecrawl 客户端。
    pub fn new(api_key: String) -> Self {
        let endpoint = env::var("FIRECRAWL_SCRAPE_ENDPOINT")
            .unwrap_or_else(|_| "https://api.firecrawl.dev/v2/scrape".to_string());

        Self {
            client: reqwest::Client::new(),
            api_key,
            endpoint,
        }
    }
}

#[async_trait]
impl HttpClient for FirecrawlClient {
    async fn get(&self, url: &str, headers: &HashMap<String, String>) -> Result<Response> {
        let mut req = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&json!({
                "url": url,
                "formats": ["markdown", "html"],
                "onlyMainContent": true
            }));

        for (key, value) in headers {
            req = req.header(key, value);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CrawlError::Http(format!("Firecrawl 请求失败: {e}")))?;

        let status = resp.status().as_u16();
        let raw = resp
            .text()
            .await
            .map_err(|e| CrawlError::Http(format!("读取 Firecrawl 响应失败: {e}")))?;

        let body = firecrawl_body(&raw).unwrap_or(raw);
        let mut response_headers = HashMap::new();
        response_headers.insert(
            "content-type".to_string(),
            "text/markdown; charset=utf-8".to_string(),
        );

        Ok(Response {
            url: url.to_string(),
            status,
            headers: response_headers,
            body,
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

    fn name(&self) -> &str {
        "firecrawl"
    }
}

fn firecrawl_body(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let data = value.get("data").unwrap_or(&value);

    data.get("markdown")
        .and_then(Value::as_str)
        .or_else(|| data.get("html").and_then(Value::as_str))
        .or_else(|| data.get("content").and_then(Value::as_str))
        .map(str::to_string)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = match env::var("FIRECRAWL_API_KEY") {
        Ok(api_key) => api_key,
        Err(_) => {
            println!("请先设置 FIRECRAWL_API_KEY");
            return Ok(());
        }
    };

    let client = FirecrawlClient::new(api_key);
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
