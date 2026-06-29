//! 爬取 Rust 中文社区（rustcc.cn）最新文章
//!
//! 分页获取文章列表 → 提取链接 → 逐篇抓取 → Readability 转 Markdown。
//!
//! 运行：`cargo run --example rustcc_news`

use crawlkit::html;
use crawlkit::{Collector, ReqwestClient};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ReqwestClient::builder()
        .timeout(Duration::from_secs(30))
        .random_user_agent(true)
        .build()?;
    let collector = Collector::with_client(client);

    // 爬取前 3 页，收集所有文章链接
    let mut all_links = Vec::new();
    for page in 1..=3 {
        let url = format!("https://rustcc.cn/latest_articles_paging?current_page={page}");
        println!("第 {page} 页: {url}");

        match collector.get_links(&url, "ul li span.left a.title").await {
            Ok(links) => {
                println!("  获取 {} 个链接", links.len());
                all_links.extend(links);
            }
            Err(e) => eprintln!("  失败: {e}"),
        }
    }

    all_links.sort();
    all_links.dedup();
    println!("\n共 {} 篇文章\n", all_links.len());

    // 逐篇抓取并用 Readability 提取 Markdown
    for (i, link) in all_links.iter().enumerate() {
        print!("[{}/{}] {}", i + 1, all_links.len(), link);
        let resp = match collector.client().get(link, &HashMap::new()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  [请求失败] {e}");
                continue;
            }
        };
        match html::extract_readable_content(&resp.body) {
            Ok(md) => println!("  [{} 字符]", md.len()),
            Err(e) => println!("  [Readability 失败] {e}"),
        }
    }

    Ok(())
}
