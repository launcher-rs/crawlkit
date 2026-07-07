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
│   ├── crawlkit-media/         # 媒体提取（图片/视频/音频/文档/嵌入）与下载
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

### 媒体提取（需 `media` feature，默认启用）

```rust
use crawlkit::media::{MediaExtractor, MediaDownloader};

// 从 HTML 提取媒体
let extractor = MediaExtractor::new().with_base_url("https://example.com");
let media = extractor.extract_all(html)?;
println!("图片: {}, 视频: {}", media.images.len(), media.videos.len());

// 下载媒体文件
let downloader = MediaDownloader::default();
let result = downloader.download("https://example.com/image.jpg").await?;
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

## parser 模块（集成自 halldyll-parser）

crawlkit-parser 基于 halldyll-parser 的 14 个模块，文件结构：
- **types.rs**: 共享类型系统（ParserConfig, TextContent, Link, etc.）
- **selector.rs**: 预编译 CSS 选择器（CachedSelectors）+ 运行时缓存
- **metadata.rs**: 元数据/OG/Twitter/JSON-LD/robots/noscript 提取（7 个公共函数 + 30+ 内部辅助）
- **text.rs**: 正文提取/可读性/语言检测
- **links.rs**: 链接提取/URL 规范/分类
- **content.rs**: 标题/段落/列表/表格/代码块/引用/图片提取
- **forms.rs**: 表单检测与分类
- **pagination.rs**: 翻页检测（数字/上页下页/无限滚动/游标/偏移量）
- **contact.rs**: 联系方式提取（邮箱/电话/地址/社媒链接）
- **feeds.rs**: RSS/Atom/Sitemap 检测（32 个常见 Feed 路径 + 26 个 sitemap 路径）
- **fingerprint.rs**: 内容指纹/AMP 检测/缓存建议
- **parser.rs**: HtmlParser 统一入口（orchestrator）
- **html.rs**: 向后兼容（原有 extract_links/extract_article 等 API）
- **lib.rs**: 统一重导出所有模块

### 注意事项
- **extract_links 冲突**：html.rs 版返回 `Vec<String>`，links.rs 版返回 `Vec<Link>`；lib.rs 只 re-export html.rs 版，links.rs 版通过 `links::extract_links` 访问
- **extract_element_text / should_skip_element**：text.rs 中有 4 个 dead_code 警告（保留辅助函数，未被主流程调用）
- **selectors 模块**：不对应 halldyll-parser 的 selectors 文件；`selectors` crate 已集成到 scraper，改用 `scraper::Selector` + 预编译缓存模式
- **依赖**: serde/serde_json/thiserror/regex/lazy_static
