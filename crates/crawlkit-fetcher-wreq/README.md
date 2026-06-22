# crawlkit-fetcher-wreq

基于 wreq 的 HTTP 客户端实现。wreq 是 reqwest 的硬分叉，支持 TLS 指纹模拟（JA3/JA4/Akamai）、请求重试、代理配置。

## 依赖

wreq 依赖 `btls-sys`（BoringSSL），编译需要 **NASM** 汇编器和 **Visual Studio**（`vcvars64.bat`）。

```powershell
# Windows: 安装 NASM
winget install NASM.NASM
```

## Cargo 配置

```toml
crawlkit-fetcher-wreq = { path = "../crawlkit-fetcher-wreq" }
```

## 使用

```rust
use crawlkit_fetcher_wreq::WreqClient;

let client = WreqClient::builder()
    .timeout(std::time::Duration::from_secs(60))
    .user_agent("MyBot/1.0")
    .build()?;
```

### 浏览器指纹模拟

通过 `wreq-util` 提供的预定义浏览器配置，可模拟 Chrome、Firefox、Safari、Edge、Opera、OkHttp 等：

```rust
use wreq_util::{Emulation, Platform, Profile};

let client = WreqClient::builder()
    .user_agent("Mozilla/5.0 ...")
    .emulation(Emulation::builder()
        .profile(Profile::Chrome130)
        .platform(Platform::Windows)
        .build())
    .build()?;
```

### 随机指纹

```rust
use wreq_util::Emulation;

let client = WreqClient::builder()
    .emulation(Emulation::random())          // 完全随机
    // .emulation(Emulation::weighted_random()) // 按市场占有率加权随机
    .build()?;
```

## 功能

- TLS 指纹模拟（JA3/JA4/Akamai），支持 Chrome / Firefox / Safari / Edge / Opera / OkHttp
- 代理配置（环境变量 `PROXY_URL` / `PROXY_USER` / `PROXY_PASS`）
- 指数退避重试（默认 3 次）
- 可配置超时、User-Agent
- 随机浏览器指纹（`Emulation::random()` / `Emulation::weighted_random()`）

通常通过 `crawlkit` facade crate（需启用 `fetcher-wreq` 特性）访问。
