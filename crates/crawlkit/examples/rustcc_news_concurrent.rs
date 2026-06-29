//! 并发爬取 Rust 中文社区（rustcc.cn）最新文章
//!
//! 演示 Colly 风格的多 Collector 协作 + `Arc<Collector>::run()` 并发调度：
//! 1. 主 Collector（`let mut`）：配置 UA/限速，爬列表页提取链接
//! 2. `Arc<Collector>::run()`：共享 HTTP 后端 + 去重，限速并发
//!
//! 运行：`cargo run --example rustcc_news_concurrent`

use crawlkit::html::{self, LinkSelectorType};
use crawlkit::{Collector, LimitRule, ReqwestClient};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 主 Collector：配置后端、限速规则
    let mut c = Collector::with_client(
        ReqwestClient::builder()
            .timeout(Duration::from_secs(30))
            .random_user_agent(true)
            .build()?,
    );
    c.add_limit(LimitRule {
        domain_glob: "*rustcc.cn*".into(),
        parallelism: 3,
        delay: Duration::from_millis(800),
        ..Default::default()
    });

    // 2. 爬前 3 页，收集文章链接
    let mut all_links = Vec::new();
    for page in 1..=3 {
        let url = format!("https://rustcc.cn/latest_articles_paging?current_page={page}");
        print!("第 {page} 页...");
        let resp = c.client().get(&url, &HashMap::new()).await?;
        let links = html::extract_absolute_links(
            &resp.body,
            "ul li span.left a.title",
            LinkSelectorType::Css,
            &resp.url,
        )?;
        println!(" {} 个链接", links.len());
        all_links.extend(links);
    }
    all_links.sort();
    all_links.dedup();
    println!("\n共 {} 篇文章，开始并发爬取...\n", all_links.len());

    // 3. Arc + run()：共享 visited 去重，LimitRule 控制并发
    let c = Arc::new(c);
    let results = c.run(all_links).await;

    // 4. 用 Readability 提取正文
    let mut ok = 0;
    let mut fail = 0;
    for (url, result) in &results {
        match result {
            Ok(resp) => match html::extract_readable_content(&resp.body) {
                Ok(md) => {
                    let preview = if md.len() > 80 {
                        format!("{}...", &md[..80])
                    } else {
                        md.clone()
                    };
                    println!("  ✓ [{} 字符] {}", md.len(), preview.replace('\n', " "));
                    ok += 1;
                }
                Err(e) => {
                    println!("  ✗ Readability 失败 [{}] {}", e, url);
                    fail += 1;
                }
            },
            Err(e) => {
                println!("  ✗ 请求失败 [{}] {}", e, url);
                fail += 1;
            }
        }
    }

    println!("\n完成：成功 {ok} / 失败 {fail}");
    Ok(())
}
