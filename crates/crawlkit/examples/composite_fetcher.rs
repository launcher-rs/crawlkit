//! 组合请求器示例
//!
//! 演示如何使用 CompositeFetcher 组合多个 HTTP 客户端。

use std::collections::HashMap;

use crawlkit::{CompositeFetcher, ReqwestClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 组合请求器示例 ===\n");

    // 1. 创建多个客户端
    let client1 = ReqwestClient::builder()
        .name("primary")
        .max_retries(2)
        .build()?;

    let client2 = ReqwestClient::builder()
        .name("fallback")
        .timeout(std::time::Duration::from_secs(60))
        .max_retries(1)
        .build()?;

    // 2. 创建组合请求器
    let fetcher = CompositeFetcher::new(vec![
        Box::new(client1),
        Box::new(client2),
    ]);

    println!("组合请求器包含 {} 个客户端", fetcher.len());

    // 3. 使用组合请求器获取内容
    let url = "https://httpbin.org/get";
    let headers = HashMap::new();

    println!("\n尝试获取: {}", url);
    match fetcher.get(&url, &headers).await {
        Ok(response) => {
            println!("获取成功！");
            println!("状态码: {}", response.status);
            println!("内容长度: {} 字节", response.body.len());
            println!("Content-Type: {:?}", response.content_type());
        }
        Err(e) => println!("获取失败: {}", e),
    }

    // 4. 作为 HttpClient 使用
    println!("\n--- 作为 HttpClient trait 使用 ---");
    let resp = fetcher.get("https://httpbin.org/ip", &HashMap::new()).await?;
    println!("IP 地址: {}", resp.body);

    Ok(())
}
