//! # 组合请求器示例
//!
//! 演示 CompositeFetcher 的故障转移 + 机器人验证自动切换：
//! - 多个 HTTP 客户端按优先级依次尝试
//! - 自动检测机器人验证页面（PerimeterX / Cloudflare / DataDome 等）
//! - 检测到验证页面时自动切换到下一个后端
//! - `on_backend_error` 回调跟踪每个后端的失败原因
//!
//! 运行：`cargo run --example composite_fetcher`

use std::collections::HashMap;

use crawlkit::{CompositeFetcher, ReqwestClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== 组合请求器示例 ===\n");

    // 1. 创建多个客户端（不同配置模拟不同后端）
    let client1 = ReqwestClient::builder()
        .name("primary")
        .max_retries(2)
        .build()?;

    let client2 = ReqwestClient::builder()
        .name("fallback")
        .timeout(std::time::Duration::from_secs(60))
        .max_retries(1)
        .build()?;

    // 2. 创建组合请求器，启用机器人验证检测 + 注册失败回调
    let fetcher = CompositeFetcher::new(vec![Box::new(client1), Box::new(client2)])
        // 启用机器人验证检测（默认已开启，这里显式演示）
        .detect_bot_challenge(true)
        // 每个后端失败时触发（网络错误或机器人验证）
        .on_backend_error(|name, err| {
            eprintln!("  [后端失败] {}: {}", name, err);
        });

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
        }
        Err(e) => println!("获取失败: {}", e),
    }

    // 4. 对抗机器人验证场景
    println!("\n--- 对抗机器人验证 ---");
    let protected_url = "https://thehill.com/news/";
    println!("尝试获取受保护页面: {}", protected_url);
    println!("（如果 primary 被拦截，会自动切换到 fallback）\n");

    match fetcher.get(&protected_url, &headers).await {
        Ok(response) => {
            println!("获取成功！状态码: {}, 长度: {} 字节", response.status, response.body.len());
        }
        Err(e) => println!("所有后端均失败: {}", e),
    }

    Ok(())
}
