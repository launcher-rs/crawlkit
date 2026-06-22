//! # 示例 2：直接提取文章
//!
//! 一步到位：给定 URL，直接提取文章标题、正文、日期等。
//!
//! 运行：`cargo run --example extract_article`

use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let c = Collector::new();

    let url = "https://news.ycombinator.com/news";
    match c.get_article(url).await {
        Ok(article) => {
            println!("标题: {}", article.title);
            if let Some(desc) = &article.description {
                println!("描述: {}", desc);
            }
            if let Some(date) = &article.date {
                println!("日期: {}", date);
            }
            if let Some(author) = &article.author {
                println!("作者: {}", author);
            }
            if !article.content.is_empty() {
                let preview = if article.content.len() > 500 {
                    &article.content[..500]
                } else {
                    &article.content
                };
                println!("正文预览:\n{}", preview);
            } else {
                println!("（未能提取到正文，可能需要自定义选择器）");
            }
        }
        Err(e) => {
            eprintln!("抓取失败: {}", e);
        }
    }
}
