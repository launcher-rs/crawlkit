//! # 组合请求器示例
//!
//! 演示 CompositeFetcher 的故障转移 + 拦截自动切换：
//! - 多个 HTTP 客户端按优先级依次尝试
//! - 自动检测机器人验证页面（PerimeterX / Cloudflare / DataDome 等）
//! - 自动检测访问被拒绝页面（Akamai CDN / Cloudflare WAF 等）
//! - 自定义检测逻辑（适配特殊拦截场景）
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

    // 2. 创建组合请求器，配置检测 + 注册回调
    let fetcher = CompositeFetcher::new(vec![Box::new(client1), Box::new(client2)])
        // ── 内置检测（默认已开启，这里显式演示） ──
        // 自动检测 PerimeterX / Cloudflare challenge 等机器人验证页面
        .detect_bot_challenge(true)
        // 自动检测 Akamai CDN / Cloudflare WAF 等403拒绝页面
        .detect_access_denied(true)
        // ── 自定义检测（可选，与内置检测叠加生效） ──
        // 自定义机器人验证检测：适配业务特定的验证页面
        .on_detect_bot_challenge(|resp| {
            let lower = resp.body.to_ascii_lowercase();
            // 检测自定义验证页面特征
            lower.contains("please complete the verification")
                || lower.contains("custom-captcha-widget")
        })
        // 自定义访问被拒绝检测：适配企业内部 WAF
        .on_detect_access_denied(|resp| {
            resp.status == 403
                && (resp.body.contains("corporate security block")
                    || resp.body.contains("internal firewall denied"))
        })
        // 后端失败回调（网络错误或拦截检测均触发）
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
    println!("（如果 primary 被 PerimeterX 拦截，会自动切换到 fallback）\n");

    match fetcher.get(&protected_url, &headers).await {
        Ok(response) => {
            println!(
                "获取成功！状态码: {}, 长度: {} 字节",
                response.status,
                response.body.len()
            );
        }
        Err(e) => println!("所有后端均失败: {}", e),
    }

    // 5. 对抗访问被拒绝场景
    println!("\n--- 对抗访问被拒绝 ---");
    let blocked_url = "https://www.dhs.gov/all-news-updates";
    println!("尝试获取受保护页面: {}", blocked_url);
    println!("（如果 primary 被 Akamai CDN 拒绝，会自动切换到 fallback）\n");

    match fetcher.get(&blocked_url, &headers).await {
        Ok(response) => {
            println!(
                "获取成功！状态码: {}, 长度: {} 字节",
                response.status,
                response.body.len()
            );
        }
        Err(e) => println!("所有后端均失败: {}", e),
    }

    Ok(())
}
