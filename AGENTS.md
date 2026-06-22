# AGENTS.md — crawlkit 项目指南

## 项目结构（Workspace）

```
crawlkit/
├── Cargo.toml                  # workspace root
├── AGENTS.md                   # 本文件
├── docs/
│   └── multi-crate-research.md # 多 crate 调研报告
├── crates/
│   ├── crawlkit-core/          # 核心抽象：HttpClient trait、Request/Response、CrawlError
│   ├── crawlkit-fetcher/       # 组合/故障转移客户端（无后端依赖）
│   ├── crawlkit-fetcher-reqwest/  # reqwest 后端
│   ├── crawlkit-fetcher-wreq/     # wreq 后端（TLS 指纹模拟）
│   ├── crawlkit-parser/        # HTML 解析、Readability、文章提取
│   └── crawlkit/               # facade 入口：Collector 调度器 + 重新导出
```

## 后端架构

- **HttpClient trait** 定义在 `crawlkit-core`，是接入框架的唯一契约
- **后端子 crate** 各自独立实现 `HttpClient`，互不依赖
- **Feature gates** 控制后端按需编译（`fetcher-reqwest`、`fetcher-wreq`）
- **自定义后端**：实现 `HttpClient` trait，通过 `Collector::with_client()` 接入

## 关键 API

### Collector 构造

```rust
// 内置后端
Collector::reqwest()           // 需 feature: fetcher-reqwest（默认）
Collector::wreq()             // 需 feature: fetcher-wreq

// 自定义后端
Collector::with_client(my_client)  // 始终可用
```

### 回调链

```rust
c.on_request(|req| { /* 修改请求 */ });
c.on_response(|resp| { /* 处理响应 */ });
c.on_html(|html, url| { /* 解析 HTML */ });
c.on_error(|err| { /* 处理错误 */ });
```

## 编译环境

- wreq 依赖 `btls-sys`（BoringSSL），需要 NASM 汇编器
- Windows: `winget install NASM.NASM`
- 构建前需执行 `vcvars64.bat` 设置 VS 环境变量
- 可能需要设置 `BINDGEN_EXTRA_CLANG_ARGS=--target=x86_64-pc-windows-msvc`（LLVM 19）

## 代码规范

- 注释使用中文
- 后端子 crate 使用 `backon` 实现指数退避重试
- 环境变量配置代理：`PROXY_URL` / `PROXY_USER` / `PROXY_PASS`
- 所有公共 API 需有 Rustdoc 中文注释和文档示例
