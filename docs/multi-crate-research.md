# crawlkit 多 Crate 拆分研究报告

> 日期：2026-06-22
> 目标：将 crawlkit 从单 crate 拆分为模块化的多 crate workspace 架构

---

## 1. 背景

当前 `crawlkit` 是一个单 crate 项目 (`src/` 下 9 个模块)。虽然功能尚可，但随着需要支持多种 HTTP 后端（reqwest、wreq、Chrome CDP）、多种解析策略、中间件链、数据管道等需求增长，单 crate 架构的局限日益明显：

- **编译慢**：修改任一模块都会触发整体重编
- **依赖膨胀**：用户被迫引入所有依赖（reqwest、scraper、dom_smoothie 等），即使只需要其中一部分
- **难以扩展**：无法按需选择功能子集
- **发布粒度粗**：第三方贡献者无法独立发布中间件或解析器扩展

---

## 2. 业界调研

调研了 Rust 生态中主流爬虫框架的 crate 组织结构，概要如下：

| 项目 | Stars | 架构特点 |
|------|-------|----------|
| **spider-rs** | 2.5k | 单 crate + feature flags，最小化默认构建 |
| **spider-lib** | — | Scrapy 风格，6 个子 crate + 1 个 facade crate |
| **kreuzcrawl** | — | 核心 + FFI 绑定，feature flags 控制能力 |
| **Argus** | — | 10+ 子 crate，极度细粒度拆分 |
| **omnivore** | — | 3 子 crate（core + cli + api） |
| **halldyll** | — | 5 子 crate（core + parser + media + robots + python） |
| **stygian** | — | 5 子 crate，六边形架构 |

---

### 2.1 趋势总结

1. **Facade/Umbrella 模式**：根 crate 作为外观，重新导出子 crate，普通用户只需 `use crawlkit::*`
2. **Feature flags 控制深度**：默认最小化依赖，按需启用 chrome、tracing、mcp 等
3. **核心与基础设施分离**：核心类型/接口零 I/O 依赖，具体的 HTTP 客户端、存储后端作为独立 crate
4. **Pipeline/中间件可插拔**：每个中间件或管道设计为独立 crate
5. **Workspace 统一管理**：所有子 crate 在同一 repo 下，共享版本和 CI

---

## 3. 建议的 Crate 拆分方案

### 3.1 Crate 依赖关系图

```
crawlkit (facade crate - 用户入口)
├── crawlkit-core          ← 核心类型、trait、错误定义
├── crawlkit-fetcher       ← HTTP 客户端抽象 + 实现（reqwest）
├── crawlkit-parser        ← HTML 解析、CSS 选择器、可读性提取
├── crawlkit-middleware    ← 中间件链（retry, rate-limit, robots, UA）
├── crawlkit-pipeline      ← 数据处理管道（JSON, CSV, storage）
├── crawlkit-frontier      ← URL 调度、优先级队列、去重
├── crawlkit-robots        ← robots.txt 解析和缓存
└── crawlkit-storage       ← 存储后端抽象
```

### 3.2 各 Crate 职责

#### `crawlkit-core`

```
目的：零外部依赖的核心类型定义
依赖：无（或仅 thiserror）
内容：
  - Request / Response 结构体
  - HttpClient trait（获取/发布接口）
  - Collector 核心调度逻辑
  - CollyError / Result 类型
  - Callback 类型定义
  - ScrapeStats / ScrapedArticle / SiteConfig
```

- 此为整个生态的"契约"，变更需谨慎
- 不依赖任何 HTTP 库、HTML 解析库

#### `crawlkit-fetcher`

```
目的：HttpClient trait 的具体实现
依赖：crawlkit-core, reqwest (可选), wreq (可选), headless_chrome (可选)
内容：
  - ReqwestClient (default feature)
  - WreqClient (feature = "wreq")
  - ChromeClient (feature = "chrome")
  - CompositeFetcher
```

- 每个后端通过 feature flag 控制
- 支持链式回退（CompositeFetcher）

#### `crawlkit-parser`

```
目的：HTML 内容提取和分析
依赖：crawlkit-core, scraper, dom_smoothie, url
内容：
  - extract_links / resolve_url
  - extract_article（启发式提取）
  - extract_readable_content（Readability 模式）
  - extract_content_by_selector
  - extract_texts / extract_attributes
  - Article 结构体
```

#### `crawlkit-middleware`

```
目的：请求/响应处理中间件
依赖：crawlkit-core
内容：
  - RetryMiddleware：指数退避重试
  - RateLimitMiddleware：域级别限速
  - RobotsMiddleware：robots.txt 合规
  - UserAgentMiddleware：UA 轮换
  - CookieMiddleware：持久化 Cookie
  - ProxyMiddleware：代理轮换
  - LogMiddleware：结构化日志
```

- 每个中间件可选 feature
- 参考 `tower::Layer` 模式组合

#### `crawlkit-pipeline`

```
目的：提取后数据流的处理与输出
依赖：crawlkit-core, serde, csv, rusqlite (可选)
内容：
  - JsonPipeline：JSON 输出
  - CsvPipeline：CSV 输出
  - SqlitePipeline：SQLite 存储
  - ConsolePipeline：终端打印
  - CustomPipeline：用户自定义
```

#### `crawlkit-frontier`

```
目的：URL 调度策略
依赖：crawlkit-core
内容：
  - MemoryFrontier：内存队列
  - PriorityFrontier：优先级队列（Mercator 双队列）
  - RedisFrontier：Redis 后端（分布式）
  - DeduplicationFilter：Bloom 过滤器去重
  - PolitenessPolicy：每个主机的礼貌延迟
```

- 参考 Mercator 论文的双队列设计
- 默认使用内存实现

#### `crawlkit-robots`

```
目的：Robots.txt 解析与合规检查
依赖：crawlkit-core, regex
内容：
  - RobotsParser：解析 robots.txt
  - RobotsCache：缓存解析结果
  - RobotsMiddleware：集成到中间件链
```

#### `crawlkit-storage`

```
目的：数据持久化抽象
依赖：crawlkit-core
内容：
  - Storage trait：save / load / list / delete
  - FileStorage：本地文件系统
  - S3Storage：S3/MinIO（可选）
  - DatabaseStorage：关系数据库（可选）
```

#### `crawlkit`（Facade Crate）

```
目的：用户唯一需引用的入口 crate
依赖：所有子 crate（按 feature 可选）
角色：
  - 重新导出公共 API
  - 通过 feature flags 控制依赖树
  - 提供预导出的 `prelude` 模块
```

### 3.3 Feature Flags 设计

| Feature | 依赖 | 启用内容 |
|---------|------|----------|
| `default` | core + fetcher(reqwest) + parser | 基础爬虫功能 |
| `full` | 所有 crate + 所有后端 | 全功能模式 |
| `chrome` | crawlkit-fetcher(chrome) | Headless Chrome 渲染 |
| `wreq` | crawlkit-fetcher(wreq) | wreq HTTP 后端 |
| `middleware` | crawlkit-middleware | 开启默认中间件 |
| `pipeline-json` | crawlkit-pipeline(json) | JSON 管道 |
| `pipeline-csv` | crawlkit-pipeline(csv) | CSV 管道 |
| `pipeline-sqlite` | crawlkit-pipeline(sqlite) | SQLite 管道 |
| `frontier-redis` | crawlkit-frontier(redis) | Redis URL 队列 |
| `robots` | crawlkit-robots | Robots.txt 支持 |
| `storage-s3` | crawlkit-storage(s3) | S3 存储 |
| `tracing` | tracing crate | OpenTelemetry 支持 |
| `api` | axum | REST API 服务器 |
| `mcp` | — | Model Context Protocol 支持 |

---

## 4. 迁移路径

### 第一阶段：建立 Workspace 骨架（1-2天）

```
crawlkit/
├── Cargo.toml              # workspace [members]
├── crates/
│   ├── crawlkit-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── crawlkit-fetcher/
│   ├── crawlkit-parser/
│   └── crawlkit/           # facade
├── src/                    # 旧代码（迁移后删除）
├── examples/
└── docs/
```

1. 创建 `cargo workspace`
2. 创建 `crawlkit-core`，从 `src/` 提取核心类型
3. 创建 `crawlkit-fetcher`，迁移 `client.rs`, `fetcher.rs`
4. 创建 `crawlkit-parser`，迁移 `html.rs`
5. 创建 facade crate `crawlkit`，重新导出

### 第二阶段：提取现有功能（2-3天）

1. 将 `Collector` 重构为仅依赖 `crawlkit-core` + `crawlkit-fetcher`
2. 将错误类型拆分，`CollyError` 变体按 crate 分布
3. 更新示例代码使用新的 crate 结构
4. 确保所有测试通过

### 第三阶段：新增功能（持续）

1. 创建 `crawlkit-middleware`
2. 创建 `crawlkit-pipeline`
3. 创建 `crawlkit-frontier`
4. 创建 `crawlkit-robots`
5. 创建 `crawlkit-storage`
6. 新增 feature flags 控制

---

## 5. 关键设计决策

### 5.1 `Collector` 的位置

`Collector` 作为核心调度器，放在 **`crawlkit-core`** 中。它只依赖 `HttpClient` trait（定义在 core），不依赖具体实现。

```rust
// crawlkit-core 中的 Collector
pub struct Collector<C: HttpClient> {
    client: Arc<C>,
    // ...
}
```

### 5.2 Error 类型设计

- `crawlkit-core` 定义 `CoreError`（基础错误）
- 每个子 crate 定义自己的 `XxxError`
- Facade crate 提供统一的 `CrawlkitError` 枚举聚合所有错误

### 5.3 版本策略

- 所有 crate 保持同一主版本号
- 使用 `workspace.package.version` 统一管理
- 建议 `0.2.0` 开始拆分

### 5.4 可选：tower 集成

借鉴 `recluse` 和 `tower` 生态，中间件可以用 `tower::Layer` 组合：

```rust
let service = RetryLayer::new(3)
    .layer(RateLimitLayer::per_host(Duration::from_secs(1)))
    .layer(LogLayer);
```

---

## 6. 性能考量

多 crate 拆分不会引入运行时开销（Rust 的 crate 边界在编译时消除），但带来以下编译期收益：

| 场景 | 单 crate 编译 | 多 crate 编译 |
|------|--------------|--------------|
| 仅改动 parser | 全量重编 | 仅 parser 和 facade 重编 |
| 添加新 HTTP 后端 | 全量重编 | 仅 fetcher 和 facade 重编 |
| 首次构建 | 相同 | 略慢（更多 crate 元数据） |
| 增量构建 | 慢 | **显著更快** |

---

## 7. 参考项目

- [spider-rs/spider](https://github.com/spider-rs/spider) - 2.5k stars，特征标志驱动的爬虫引擎
- [spider-lib](https://crates.io/crates/spider-lib) - Scrapy 风格，6 子 crate 架构
- [Argus](https://github.com/dedsecrattle/argus) - 10+ 子 crate，生产级分布式爬虫
- [kreuzcrawl](https://docs.kreuzcrawl.kreuzberg.dev/) - 核心 + FFI 绑定模式
- [halldyll](https://docs.rs/crate/halldyll-core/latest) - core + parser + media + robots + python
- [recluse](https://docs.rs/recluse/latest/) - tower 层组合的爬虫框架
- [stygian](https://github.com/greysquirr3l/stygian) - 六边形架构，5 子 crate
- [Scrapy Architecture](https://docs.scrapy.org/en/latest/topics/architecture.html) - Python 生态参考
- [Mercator 论文](https://doi.org/10.1016/S0169-7552(02)00144-X) - 双队列 URL 调度

---

## 8. 结论

建议 crawlkit 按 **facade + 6-8 子 crate** 的架构重构：

1. **立即执行的短期收益**：拆出 `crawlkit-core` 和 `crawlkit-fetcher`，这 2 个 crate 覆盖了 80% 的依赖解耦需求
2. **中期目标**：`crawlkit-parser`、`crawlkit-middleware`、`crawlkit-pipeline`，实现中间件链和输出管道
3. **长期扩展**：`crawlkit-frontier`、`crawlkit-robots`、`crawlkit-storage`，支持分布式爬取和多种存储后端

这种架构在保持 `use crawlkit::*` 简单入口的同时，提供了极致的灵活性和编译速度。
