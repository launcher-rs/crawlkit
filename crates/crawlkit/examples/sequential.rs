//! # 示例：顺序请求模式（非回调）
//!
//! 演示如何一步步地：获取页面 → 解析链接 → 请求每个链接。
//! 全程使用普通 async/await，不需要注册任何回调。
//!
//! 运行：`cargo run --example sequential`

use std::collections::HashMap;

use crawlkit::html::{ExtractorConfig, LinkExtractor};
use crawlkit::HttpClient;

#[tokio::main]
async fn main() {
    let client = crawlkit_fetcher_reqwest::ReqwestClient::new();

    // ── 第 1 步：获取列表页 ──
    let url = "https://news.ycombinator.com/";
    println!("1. 请求列表页: {url}");
    let resp = client.get(url, &HashMap::new()).await.unwrap();
    println!("   状态: {}, 大小: {} 字节", resp.status, resp.body.len());

    // ── 第 2 步：从 HTML 中提取文章链接 ──
    let extractor = LinkExtractor::new(ExtractorConfig::default());
    let links = extractor.extract(&resp.body, url);

    println!("\n2. 提取到 {} 个文章链接:", links.len());
    for link in links.iter().take(5) {
        println!("   [{:.2}] {} — {}", link.score, link.url, link.text);
    }

    // ── 第 3 步：逐个请求文章页 ──
    println!("\n3. 逐个请求文章详情:");
    for link in links.iter().take(3) {
        let article_resp = client.get(&link.url, &HashMap::new()).await.unwrap();
        let article = crawlkit::html::extract_article(&article_resp.body, &link.url);
        println!(
            "   标题: {} | 正文: {} 字 | 日期: {:?}",
            article.title,
            article.content.len(),
            article.date,
        );
    }
}
