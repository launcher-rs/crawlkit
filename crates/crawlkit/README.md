# crawlkit

受 Go [colly](https://github.com/gocolly/colly) 启发的 Rust 爬虫工具包。

本 crate 为集成入口（facade），重新导出所有子 crate 的公共 API。大多数用户只需依赖此 crate。

## 设计理念

- **可插拔的 HTTP 客户端**：默认 reqwest，支持代理配置和重试
- **回调驱动**：类似 colly 的 `OnHTML` / `OnRequest` / `OnResponse` 模式
- **异步就绪**：基于 tokio + async-trait，支持并发爬取
- **智能内容提取**：支持 Readability 模式和 CSS 选择器提取

## 快速上手

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

## 架构

```
crawlkit (facade) ───┬── crawlkit-core    ← 核心类型、trait、错误
                     ├── crawlkit-fetcher  ← HTTP 客户端实现
                     └── crawlkit-parser   ← HTML 解析与内容提取
```

## 运行示例

```bash
cargo run --example callback        # 回调链模式
cargo run --example extract_article  # 文章提取
cargo run --example batch_crawl      # 批量并发抓取
cargo run --example custom_client    # 自定义 HTTP 客户端
cargo run --example readability      # Readability 提取
cargo run --example composite_fetcher # 组合请求器
```
