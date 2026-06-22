# crawlkit

一个受 Go [colly](https://github.com/gocolly/colly) 启发的 Rust 爬虫工具包。

## 特性

- **可插拔的 HTTP 客户端**：默认使用 reqwest，支持代理配置和重试机制
- **回调驱动**：类似 colly 的 OnHTML / OnRequest / OnResponse 模式
- **异步就绪**：基于 tokio + async-trait，支持并发爬取
- **智能内容提取**：支持 Readability 模式和 CSS 选择器提取

## 快速开始

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::new();
    c.on_request(|req| {
        println!("即将请求: {}", req.url);
    });
    c.visit("https://example.com").await.unwrap();
}
```

## 功能示例

### 回调模式

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::new();
    c.on_request(|req| println!("请求: {}", req.url));
    c.on_response(|resp| println!("响应: {}", resp.status));
    c.on_html(|html, url| println!("收到 HTML: {} ({} bytes)", url, html.len()));
    c.visit("https://example.com").await.unwrap();
}
```

### 提取文章

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let c = Collector::new();
    let article = c.get_article("https://example.com/article").await?;
    println!("标题: {}", article.title);
    println!("内容: {}", article.content);
    Ok(())
}
```

### 批量抓取

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let c = Collector::new();
    let links = c.get_links("https://example.com", "a[href]").await?;
    let articles = c.get_articles(&links).await;
    for article in articles {
        match article {
            Ok(a) => println!("{}: {}", a.title, &a.content[..100.min(a.content.len())]),
            Err(e) => eprintln!("错误: {}", e),
        }
    }
    Ok(())
}
```

### 可读性提取

```rust
use crawlkit::html;

let html = r#"<html><body><article><p>长文本内容...</p></article></body></html>"#;
let content = html::extract_readable_content(html)?;
```

### 组合请求器

```rust
use std::collections::HashMap;
use crawlkit::client::ReqwestClient;
use crawlkit::fetcher::CompositeFetcher;

let client1 = ReqwestClient::builder().name("primary").build()?;
let client2 = ReqwestClient::builder().name("fallback").build()?;

let fetcher = CompositeFetcher::new(vec![Box::new(client1), Box::new(client2)]);
let response = fetcher.get("https://example.com", &HashMap::new()).await?;
```

### 代理配置

通过环境变量：

```bash
export PROXY_URL="http://proxy.example.com:8080"
export PROXY_USER="username"
export PROXY_PASS="password"
```

或通过构建器：

```rust
use crawlkit::ReqwestClient;

let client = ReqwestClient::builder()
    .proxy_url("http://proxy.example.com:8080")
    .proxy_user("username")
    .proxy_pass("password")
    .build()?;
```

## API 参考

### 核心类型

| 类型 | 说明 |
|------|------|
| `Collector` | 爬虫核心调度器 |
| `HttpClient` | HTTP 客户端 trait |
| `ReqwestClient` | 默认 HTTP 客户端（基于 reqwest） |
| `CompositeFetcher` | 组合请求器，支持多客户端故障转移 |
| `Request` | 请求封装 |
| `Response` | 响应封装 |

### HTML 工具

| 函数 | 说明 |
|------|------|
| `extract_links()` | 提取链接 |
| `extract_article()` | 启发式提取文章 |
| `extract_readable_content()` | Readability 模式提取 |
| `extract_content_by_selector()` | CSS 选择器提取 |
| `extract_texts()` | 提取多个文本 |
| `extract_attributes()` | 提取属性值 |

### 数据类型

| 类型 | 说明 |
|------|------|
| `Article` | 文章内容 |
| `SiteConfig` | 网站配置 |
| `ScrapedArticle` | 抓取的文章信息 |
| `ScrapeStats` | 爬取统计 |

## 运行示例

```bash
cargo run --example callback
cargo run --example extract_article
cargo run --example batch_crawl
cargo run --example custom_client
cargo run --example readability
cargo run --example composite_fetcher
```

## 依赖

- `reqwest` - HTTP 客户端
- `scraper` - HTML 解析
- `dom_smoothie` - Readability 模式提取
- `backon` - 重试机制
- `tokio` - 异步运行时
- `async-trait` - 异步 trait 支持
- `tracing` - 结构化日志

## License

MIT
