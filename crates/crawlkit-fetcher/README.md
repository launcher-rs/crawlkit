# crawlkit-fetcher

组合请求器 `CompositeFetcher`，可依次尝试多个 `HttpClient` 实现故障转移。

## 使用

```rust
use crawlkit_fetcher::CompositeFetcher;
use crawlkit_fetcher_reqwest::ReqwestClient;

let fetcher = CompositeFetcher::new(vec![
    Box::new(ReqwestClient::builder().name("primary").build()?),
    Box::new(ReqwestClient::builder().name("fallback").build()?),
]);
```

## 子 crate

| crate | 后端 | 说明 |
|-------|------|------|
| `crawlkit-fetcher-reqwest` | reqwest | 默认 HTTP 后端，支持代理和重试 |
| `crawlkit-fetcher-wreq`（计划） | wreq | wreq HTTP 后端 |
| `crawlkit-fetcher-chrome`（计划） | Chrome CDP | Headless Chrome 渲染 |
