//! # 自定义外部服务后端组合示例
//!
//! 演示将自定义 HttpClient（Jina Reader API）与内置 ReqwestClient 组合：
//! - 优先使用本地 ReqwestClient 直接请求
//! - 失败时自动切换到 Jina Reader API（远程渲染）
//! - `on_backend_error` 跟踪切换过程
//! - 自动检测机器人验证页面
//!
//! 运行：`cargo run -p crawlkit --example custom_service_fallback`
//!
//! 可选环境变量：
//! - `JINA_API_TOKEN`：Jina Reader API Token

use std::{collections::HashMap, env, time::Duration};

use async_trait::async_trait;
use crawlkit::{
    Collector, CompositeFetcher, CrawlError, HttpClient, ReqwestClient, Response, Result,
};

/// Jina Reader API 客户端
///
/// 将任意 URL 通过 Jina Reader API 渲染为 Markdown，可绕过部分反爬限制。
struct JinaClient {
    client: reqwest::Client,
    token: Option<String>,
}

impl JinaClient {
    fn new(token: Option<String>) -> Self {
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
    // 1. 创建本地 ReqwestClient（优先使用）
    let local = ReqwestClient::builder()
        .name("reqwest-local")
        .timeout(Duration::from_secs(10))
        .max_retries(1)
        .build()?;

    // 2. 创建 Jina Reader API 客户端（备选）
    let jina = JinaClient::new(env::var("JINA_API_TOKEN").ok());

    // 3. 组合：优先本地 → 失败时切换到 Jina
    let fetcher = CompositeFetcher::new(vec![Box::new(local), Box::new(jina)])
        .on_backend_error(|name, err| {
            eprintln!("  [{}] 失败: {}，切换到下一个后端", name, err);
        });

    let collector = Collector::with_client(fetcher);

    // 4. 使用 Collector 回调链处理
    let mut collector = collector;
    collector.on_html(|ctx| {
        println!("  [HTML] 获取到 {} 字节，URL: {}", ctx.body.len(), ctx.url);
    });

    println!("=== 自定义后端组合示例 ===\n");
    println!("优先使用本地 Reqwest，失败时切换到 Jina Reader API\n");

    match collector.visit("https://example.com").await {
        Ok(()) => println!("\n访问完成"),
        Err(e) => eprintln!("\n访问失败: {}", e),
    }

    Ok(())
}
