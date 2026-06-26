//! # 示例 3：批量抓取
//!
//! 先提取页面中的链接，再并发抓取多篇文章内容。
//!
//! 运行：`cargo run --example batch_crawl`

use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let c = Collector::reqwest();

    // 先提取链接
    match c
        .get_links("https://news.ycombinator.com/", ".titleline > a")
        .await
    {
        Ok(links) => {
            println!("发现 {} 个新闻链接", links.len());
            // 只抓取前 3 篇（避免过多请求）
            let batch: Vec<String> = links.into_iter().take(3).collect();
            println!("准备抓取前 {} 篇...\n", batch.len());

            // 批量并发抓取
            let results = c.get_articles(&batch).await;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(article) => {
                        println!("--- 文章 {} ---", i + 1);
                        println!("标题: {}", article.title);
                        if !article.content.is_empty() {
                            let preview = if article.content.len() > 200 {
                                &article.content[..200]
                            } else {
                                &article.content
                            };
                            println!("正文: {}...\n", preview);
                        }
                    }
                    Err(e) => {
                        eprintln!("文章 {} 抓取失败: {}\n", i + 1, e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("链接提取失败: {}", e);
        }
    }
}
