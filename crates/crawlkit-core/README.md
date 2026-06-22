# crawlkit-core

crawlkit 核心基础库。定义框架中所有公共类型、trait 和错误，**不包含任何 HTTP 或 HTML 解析依赖**，是模块间解耦的基石。

## 提供的内容

| 模块 | 说明 |
|------|------|
| `HttpClient` trait | 所有 HTTP 后端的统一接口 |
| `Request` / `Response` | 请求/响应封装 |
| `CrawlError` / `Result` | 统一错误类型 |
| `SiteConfig` | 网站配置结构 |
| `ScrapedArticle` | 抓取的文章数据 |
| `ScrapeStats` | 爬取统计 |

## 设计原则

- **零外部依赖**：核心逻辑不依赖 reqwest、scraper 等具体库
- **纯 trait 定义**：具体的 HTTP 实现在 `crawlkit-fetcher` 中
- **序列化支持**：数据结构均实现 `Serialize` / `Deserialize`

## 使用

```rust
use crawlkit_core::{HttpClient, Request, Response, CrawlError};
```

通常你不会直接使用此 crate，而是通过 `crawlkit` facade crate 访问。
