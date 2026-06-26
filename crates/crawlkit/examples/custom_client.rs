//! # 示例 4：自定义 HTTP 客户端
//!
//! 演示如何通过 Builder 模式自定义 ReqwestClient 的配置。
//!
//! 运行：`cargo run --example custom_client`

use crawlkit::{Collector, ReqwestClient};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 使用 Builder 自定义配置
    let client = ReqwestClient::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("MyBot/1.0")
        .name("my-custom-client")
        .build()
        .expect("构建客户端失败");

    let mut c = Collector::with_client(client);
    c.on_request(|req| {
        println!("  [自定义客户端] 请求: {}", req.url);
    });

    let _ = c.visit("https://httpbin.org/get").await;
    println!("自定义客户端名称: {}", c.client().name());
}
