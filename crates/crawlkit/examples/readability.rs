//! 可读性提取示例
//!
//! 演示如何使用 dom_smoothie 进行智能内容提取。

use crawlkit::html;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 示例 HTML 内容
    let html_content = r#"<!DOCTYPE html>
<html>
<head>
    <title>示例文章页面</title>
    <meta name="author" content="张三">
    <meta name="description" content="这是一篇示例文章">
</head>
<body>
    <nav>
        <a href="/">首页</a>
        <a href="/about">关于</a>
    </nav>
    <article>
        <h1>如何使用 Rust 编写爬虫</h1>
        <p class="author">作者：张三 | 2026-01-15</p>
        <div class="content">
            <p>Rust 是一门系统编程语言，以其安全性和性能著称。</p>
            <p>在编写爬虫时，Rust 可以提供出色的并发性能和内存安全保证。</p>
            <p>本文将介绍如何使用 Rust 编写一个高效的网页爬虫。</p>
            <h2>为什么选择 Rust</h2>
            <p>Rust 的所有权系统可以防止内存泄漏和数据竞争。</p>
            <p>这对于编写并发爬虫非常重要，因为爬虫需要同时处理多个网络请求。</p>
            <h2>开始编写</h2>
            <p>首先，我们需要添加必要的依赖项到 Cargo.toml。</p>
            <p>然后，我们可以使用 reqwest 库发送 HTTP 请求。</p>
            <p>最后，使用 scraper 库解析 HTML 内容。</p>
        </div>
    </article>
    <footer>
        <p>版权所有 © 2026</p>
    </footer>
</body>
</html>"#;

    println!("=== 可读性提取示例 ===\n");

    // 1. 使用可读性提取（Readability 模式）
    println!("1. 可读性提取（Readability 模式）:");
    match html::extract_readable_content(html_content) {
        Ok(content) => {
            println!("提取成功！内容长度: {} 字符", content.len());
            println!("内容预览:\n{}\n", &content[..content.len().min(500)]);
        }
        Err(e) => println!("提取失败: {}\n", e),
    }

    // 2. 使用 CSS 选择器提取
    println!("2. CSS 选择器提取:");
    match html::extract_content_by_selector(html_content, "div.content") {
        Ok(content) => {
            println!("提取成功！内容长度: {} 字符", content.len());
            println!("内容预览:\n{}\n", &content[..content.len().min(500)]);
        }
        Err(e) => println!("提取失败: {}\n", e),
    }

    // 3. 智能提取（优先可读性，失败回退选择器）
    println!("3. 智能提取（优先可读性，失败回退选择器）:");
    match html::extract_content(html_content, "div.content") {
        Ok(content) => {
            println!("提取成功！内容长度: {} 字符", content.len());
            println!("内容预览:\n{}\n", &content[..content.len().min(500)]);
        }
        Err(e) => println!("提取失败: {}\n", e),
    }

    // 4. 提取多个文本
    println!("4. 提取多个文本（所有段落）:");
    match html::extract_texts(html_content, "article p") {
        Ok(texts) => {
            println!("找到 {} 个段落:", texts.len());
            for (i, text) in texts.iter().enumerate() {
                println!("  {}: {}", i + 1, text);
            }
        }
        Err(e) => println!("提取失败: {}", e),
    }

    // 5. 提取链接
    println!("\n5. 提取所有链接:");
    let links = html::extract_links(html_content, "a[href]");
    println!("找到 {} 个链接:", links.len());
    for link in &links {
        println!("  - {}", link);
    }

    Ok(())
}
