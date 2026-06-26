# crawlkit

一个受 Go [colly](https://github.com/gocolly/colly) 启发的 Rust 爬虫工具包。

## 特性

- **可插拔的 HTTP 客户端**：默认使用 reqwest，支持代理配置和重试机制
- **回调驱动**：类似 colly 的 OnHTML / OnRequest / OnResponse 模式
- **异步就绪**：基于 tokio + async-trait，支持并发爬取
- **智能内容提取**：支持 Readability 模式和 CSS 选择器提取
- **结构化日志**：内置 tracing 支持，方便调试

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

## 回调系统

`Collector` 提供完整的回调链，按请求生命周期依次触发：

### 回调执行顺序

```
on_request → [HTTP 请求] → on_response_headers → on_response
  → [HTML 时] on_html → on_html_elements (逐个匹配) → on_xml_elements (逐个匹配)
  → [follow_links 时] 递归子链接
  → on_scraped
```

### `on_request` — 请求前

在每次 HTTP 请求发送前调用。可修改请求头、记录日志、注入认证信息等。

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::new();

    // 修改请求头
    c.on_request(|req| {
        req.headers.insert("Authorization".into(), "Bearer token".into());
        println!("[请求] {} {}", req.method, req.url);
    });

    c.visit("https://example.com").await.unwrap();
}
```

**回调签名**: `Fn(&mut Request)` — 可变引用，允许修改请求。

### `on_response_headers` — 响应头回调

收到 HTTP 响应后立即调用（早于 `on_response`），可用于检查状态码、响应头等。

```rust
c.on_response_headers(|resp| {
    println!("[响应头] {} - {}", resp.status, resp.url);
});
```

**回调签名**: `Fn(&Response)` — 不可变引用。

### `on_response` — 响应后

收到 HTTP 响应后调用（无论状态码）。可用于记录状态码、统计耗时等。

```rust
c.on_response(|resp| {
    println!("[响应] {} - {} bytes", resp.status, resp.body.len());
});
```

**回调签名**: `Fn(&Response)` — 不可变引用。

### `on_html` — HTML 解析后

当响应为 HTML 内容时调用。可用于提取链接、解析文章等。

```rust
c.on_html(|html, url| {
    println!("[HTML] {} - {} bytes", url, html.len());
});
```

**回调签名**: `Fn(&str, &str)` — (html_body, page_url)。

### `on_html_element` — CSS 选择器匹配元素

当页面中存在匹配 CSS 选择器的元素时，对每个匹配元素调用回调。可注册多次，支持多个选择器。

```rust
c.on_html_element("a[href]", |e| {
    println!("链接: {} → {}", e.text(), e.attr("href").unwrap_or(""));
});

c.on_html_element("h1", |e| {
    println!("标题: {}", e.text());
});
```

**`Element` 提供的方法**:
- `text()` — 元素的纯文本内容
- `attr(name)` — 获取属性值
- `html()` — 元素的原始 HTML
- `url` — 当前页面 URL

### `on_xml_element` — XPath 匹配元素

当页面中存在匹配 XPath 表达式的元素时，对每个匹配元素调用。

```rust
c.on_xml_element("//item/name", |e| {
    println!("名称: {}", e.text());
});
```

### `on_error` — 错误处理

请求失败时调用。可用于记录错误、触发告警等。

```rust
c.on_error(|err| {
    eprintln!("[错误] {}", err);
});
```

**回调签名**: `Fn(&dyn std::error::Error)`。

### `on_scraped` — 抓取完成

在所有回调执行完毕后触发，可用于统计、清理等收尾操作。

```rust
c.on_scraped(|resp| {
    println!("[完成] {} 处理结束", resp.url);
});
```

### 回调注意事项

- `on_request` / `on_response` / `on_html` / `on_error` / `on_response_headers` / `on_scraped` 每种只能注册一个，重复注册会覆盖前一个
- `on_html_element` / `on_xml_element` 可注册多次，支持多个选择器
- 回调执行顺序见上方流程图

## 功能示例

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

### 链接跟踪

```rust
use crawlkit::Collector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut c = Collector::new();
    c.set_follow_links(true);
    c.set_max_depth(2);               // 最大递归深度
    c.set_max_concurrency(8);         // 最大并发数
    c.on_html(|html, url| {
        println!("访问: {}", url);
    });
    c.visit("https://example.com").await?;
    Ok(())
}
```

### 可读性提取

```rust
use crawlkit::html;

let html = r#"<html><body><article><p>长文本内容...</p></article></body></html>"#;
let content = html::extract_readable_content(html)?;
```

### 组合请求器（故障转移）

```rust
use crawlkit::{CompositeFetcher, ReqwestClient};

let client1 = ReqwestClient::builder().name("primary").build()?;
let client2 = ReqwestClient::builder().name("fallback").build()?;

let fetcher = CompositeFetcher::new(vec![Box::new(client1), Box::new(client2)]);
// 按顺序尝试 client1 → client2
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

### 日志配置

```rust
// 默认 info 级别
crawlkit::log::init();

// 使用环境变量控制
// RUST_LOG=crawlkit=debug cargo run --example callback
crawlkit::log::init_with_env();

// 强制 debug 级别
crawlkit::log::init_debug();
```

## API 参考

### 核心类型

| 类型 | 说明 |
|------|------|
| `Collector` | 爬虫核心调度器 |
| `Element` | HTML/XML 元素包装器（用于选择器回调） |
| `HttpClient` | HTTP 客户端 trait |
| `ReqwestClient` | 默认 HTTP 客户端（基于 reqwest） |
| `WreqClient` | TLS 指纹模拟客户端（基于 wreq） |
| `CompositeFetcher` | 组合请求器，支持多客户端故障转移 |
| `Request` | 请求封装 |
| `Response` | 响应回调 |

### Collector 方法

| 方法 | 说明 |
|------|------|
| `new()` / `reqwest()` | 使用 reqwest 后端构建 |
| `wreq()` | 使用 wreq 后端构建 |
| `with_client()` | 使用自定义 HttpClient |
| `visit()` | 访问指定 URL |
| `get_links()` | 提取页面链接 |
| `get_article()` | 提取文章内容 |
| `get_articles()` | 批量并发抓取文章 |
| `on_request()` | 注册请求前回调 |
| `on_response_headers()` | 注册响应头回调 |
| `on_response()` | 注册响应回调 |
| `on_html()` | 注册 HTML 回调 |
| `on_html_element()` | 注册 CSS 选择器匹配元素回调 |
| `on_xml_element()` | 注册 XPath 匹配元素回调 |
| `on_error()` | 注册错误回调 |
| `on_scraped()` | 注册抓取完成回调 |

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
