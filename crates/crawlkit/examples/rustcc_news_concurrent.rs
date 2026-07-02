//! 并发爬取 Rust 中文社区（rustcc.cn）最新文章
//!
//! 演示 Colly 风格的多 Collector 协作 + `Arc<Collector>::run()` 并发调度：
//! 1. 主 Collector：配置 UA/限速/on_scraped 回调
//! 2. 爬列表页，提取文章链接
//! 3. `Arc<Collector>::run()`：共享回调 + visited 去重，限速并发
//!
//! 运行：`cargo run --example rustcc_news_concurrent`

use crawlkit::html::{self, LinkSelectorType};
use crawlkit::{Collector, LimitRule, ReqwestClient};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 用于收集爬取结果的共享容器
    let articles = Arc::new(Mutex::new(Vec::new()));

    // 1. 主 Collector：配置后端、限速、回调
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

    // 注册回调：on_scraped 在每个请求完成后触发
    let articles_clone = articles.clone();
    c.on_scraped(move |resp| {
        if resp.is_html() {
            match html::extract_readable_content(&resp.body) {
                Ok(md) => {
                    articles_clone.lock().unwrap().push((resp.url.clone(), md));
                }
                Err(e) => {
                    eprintln!("  ✗ Readability 失败 [{}] {}", e, resp.url);
                }
            }
        }
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

    // 3. Arc + run()：共享回调 + visited 去重，LimitRule 控制并发
    let c = Arc::new(c);
    c.run(all_links).await;

    // 4. 输出结果
    let results = articles.lock().unwrap();
    println!("\n完成：共 {} 篇", results.len());
    for (url, md) in results.iter() {
        let preview = if md.len() > 80 {
            format!("{}...", &md[..80])
        } else {
            md.clone()
        };
        println!(
            "  ✓ [{} 字符] {} — {}",
            md.len(),
            preview.replace('\n', " "),
            url
        );
    }

    Ok(())
}
