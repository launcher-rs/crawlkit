# crawlkit-fetcher-reqwest

基于 reqwest 的 HTTP 客户端实现。支持代理配置、指数退避重试、连接池管理。

## 使用

```rust
use crawlkit_fetcher_reqwest::ReqwestClient;

let client = ReqwestClient::builder()
    .timeout(std::time::Duration::from_secs(60))
    .user_agent("MyBot/1.0")
    .build()?;
```

## 功能

- 代理配置（环境变量 `PROXY_URL` / `PROXY_USER` / `PROXY_PASS`）
- 指数退避重试（默认 3 次）
- 可配置超时、User-Agent
- gzip / brotli / deflate / zstd 压缩支持

通常你不会直接使用此 crate，而是通过 `crawlkit` facade crate（需启用 `fetcher-reqwest` 特性）访问。
