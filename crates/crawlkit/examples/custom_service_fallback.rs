//! 自定义外部服务后端组合示例
//!
//! 运行：
//! `cargo run -p crawlkit --example custom_service_fallback`
//!
//! 可选环境变量：
//! - `JINA_API_TOKEN`：Jina Reader API Token

use std::{collections::HashMap, env, time::Duration};

use async_trait::async_trait;
use crawlkit::{
    Collector, CompositeFetcher, CrawlError, HttpClient, ReqwestClient, Response, Result,
};

pub struct JinaClient {
    client: reqwest::Client,
    token: Option<String>,
}

impl JinaClient {
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

        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let response_headers = resp
            .headers()
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp
            .text()
            .await
            .map_err(|e| CrawlError::Http(format!("读取 Jina 响应失败: {e}")))?;

        Ok(Response {
            url: final_url,
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
        "jina"
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local = ReqwestClient::builder()
        .name("reqwest-local")
        .timeout(Duration::from_secs(10))
        .max_retries(1)
        .build()?;

    let jina = JinaClient::new(env::var("JINA_API_TOKEN").ok());

    let fetcher = CompositeFetcher::new(vec![Box::new(local), Box::new(jina)]);
    let collector = Collector::with_client(fetcher);

    let response = collector
        .client()
        .get("https://example.com", &HashMap::new())
        .await?;

    println!("组合客户端: {}", collector.client().name());
    println!("状态码: {}", response.status);
    println!("内容长度: {}", response.body.len());

    Ok(())
}
