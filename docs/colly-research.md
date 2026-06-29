# Colly 优雅设计研究报告

## 一、概述

Go Colly 是 Go 生态最流行的爬虫框架，其设计哲学是 "提供简洁接口编写任意爬虫"。本文分析 Colly 的设计亮点，找出 crawlkit 可借鉴的改进方向。

## 二、核心架构对比

| 维度 | Colly | crawlkit（当前） | 差距 |
|---|---|---|---|
| Collector 构造 | `NewCollector(options...)` 函数选项模式 | `Collector::new()` / `::reqwest()` 硬编码 | Colly 更灵活 |
| 配置方式 | 公有字段 + 函数选项 + 环境变量 | Builder 模式（分散在后端 Builder） | 缺少统一配置入口 |
| 回调注册 | `OnHTML`, `OnXML`, `OnRequest` 等 | `on_html`, `on_xml_element`, `on_request` 等 | 接口类似 |
| 回调解注册 | `OnHTMLDetach`, `OnXMLDetach` | 无 | 缺少解注册能力 |
| HTTP 后端 | `WithTransport()` 替换底层 Transport | `with_client()` 替换整个 HttpClient | Colly 更细粒度 |
| 请求上下文 | `Ctx.Put`/`Ctx.Get` 贯穿请求生命周期 | `Request.context: HashMap` 已有但未充分使用 | 功能有但使用不便 |
| 并发控制 | `Async(true)` + `Wait()` | `visit()` 同步阻塞，无 Async 模式 | 缺少 Async 模式 |
| 限速 | `LimitRule{DomainGlob, Parallelism, Delay}` | `set_max_concurrency(n)` 全局的 | 缺少按域名限速 |
| 队列 | `queue.Queue` 内置工作队列 | 无 | 缺少队列调度 |
| 克隆 | `Clone()` 复制配置不复制回调 | 无 | 方便多 Collector 协作 |
| URL 过滤 | `URLFilters` / `DisallowedURLFilters` 正则 | 无 | 缺少 URL 过滤 |
| 域名白/黑名单 | `AllowedDomains` / `DisallowedDomains` | 无 | 缺少域名过滤 |
| 缓存 | `CacheDir` 文件缓存 | 无 | 缺少缓存层 |
| 存储后端 | `SetStorage()` 可替换 | 仅内存 visited 集合 | 缺少持久化 |
| 调试 | `Debugger` 接口 | `tracing` 日志 | 缺少结构化调试 |
| 反序列化 | `UnmarshalHTML` 结构体标签 | 无 | 缺少声明式提取 |
| 请求方法 | `Request(method, url, body, ctx, hdr)` | 仅 `get()` / `post()` | 缺少 PUT/DELETE 等 |
| 重试 | `Request.Retry()` 手动 + 自动 | 仅后端自动重试 | 缺少手动重试 |
| 环境变量配置 | `COLLY_*` 系列变量 | 仅代理相关 | 缺少环境变量支持 |
| 字符编码检测 | `DetectCharset` | 无 | 缺少编码检测 |

## 三、Colly 关键设计模式

### 3.1 函数选项模式（Functional Options）

```go
c := colly.NewCollector(
    colly.UserAgent("myBot/1.0"),
    colly.AllowedDomains("example.com"),
    colly.MaxDepth(2),
    colly.Async(true),
)
```

**优点**：零值友好、可扩展、IDE 自动补全友好、不变性。

**Rust 替代方案**：可以使用 Builder 模式，或者采用 TypedBuilder derive 宏。

### 3.2 限速规则（LimitRule）

```go
c.Limit(&colly.LimitRule{
    DomainGlob:  "*httpbin.*",
    Parallelism: 2,
    Delay:       5 * time.Second,
})
```

**关键设计**：
- 按域名通配符匹配，不同域名可有不同规则
- `Parallelism` 控制并发数
- `Delay`/`RandomDelay` 控制请求间隔
- 规则存储在 `[]*LimitRule` 中，请求前匹配

### 3.3 请求上下文（Context）

```go
c.OnRequest(func(r *colly.Request) {
    r.Ctx.Put("url", r.URL.String())
})
c.OnResponse(func(r *colly.Response) {
    fmt.Println(r.Ctx.Get("url"))
})
```

**作用**：跨回调传递数据，在 `OnRequest` → `OnResponse` → `OnHTML` → `OnScraped` 整个生命周期中携带自定义数据。

### 3.4 克隆模式（Clone）

```go
c := colly.NewCollector(colly.UserAgent("myUA"), colly.AllowedDomains("foo.com"))
c2 := c.Clone()  // c2 共享 UA、AllowedDomains，但无回调
```

**典型用法**：第一个 Collector 爬列表页提取链接，第二个 Collector（克隆自第一个）爬详情页。共享配置但不共享回调。

### 3.5 队列调度（Queue）

```go
q, _ := queue.New(2, &queue.InMemoryQueueStorage{MaxSize: 10000})
q.AddURL("https://example.com")
q.Run(c)
```

**特点**：
- 固定消费者线程数
- 可替换存储后端（内存 / Redis）
- Run() 阻塞直到队列为空

### 3.6 请求对象的方法链

```go
c.OnHTML("a[href]", func(e *colly.HTMLElement) {
    e.Request.Visit(e.Attr("href"))      // 从回调发起新请求
    e.Request.Abort()                     // 中止当前请求处理
    e.Request.Do()                        // 手动执行请求
    e.Request.Retry()                     // 重试
})
```

**设计亮点**：`Request` 对象持有 `Collector` 引用，可以在回调中发起新请求、控制请求生命周期。

## 四、当前 crawlkit 的核心问题

### 4.1 Collector 方法作为调度器不够灵活

`Collector::visit()` 同时负责：构造请求、发送请求、触发回调、去重、递归。用户无法方便地在不同阶段插入自定义逻辑。

### 4.2 缺少 Async 模式

所有 `visit()` 调用都是同步阻塞的，即使构建了多个 Collector 也无法并发使用。必须借助外部 `tokio::spawn` + `Semaphore`。

### 4.3 缺少按域名限速

`set_max_concurrency(n)` 是全局限制，无法对不同域名设置不同的并发和延迟。

### 4.4 缺少 Clone

Colly 的 `Clone()` 是多 Collector 协作的基础。当前如果要创建两个 Collector 爬取同一站点的不同部分，需要重复设置配置。

### 4.5 缺少请求上下文传递

虽然有 `Request.context` 字段，但 Collector 的便捷方法（`get_links`, `get_article`）无法利用上下文在回调间传递数据。

### 4.6 缺少 URL/域名过滤

无法限制爬取范围，可能误爬外部链接。

### 4.7 Rust 特有的问题

- Collector 使用 `&mut self`，限制了并发使用
- 回调是 `Fn` 而非 `FnMut`，限制了可变状态
- `Collector` 未实现 `Clone`，无法方便共享
- 后端 trait 设计过于简化（只定义了 `get` 和 `post`）

## 五、改进方案

### 5.1 短期改进（优先级高）

1. ✅ **引入 Clone**：为 Collector 实现 `Clone`（共享 HTTP 后端 + 回调，独立 visited 集）。另有 `clone_config()` 方法不复制回调，对标 Colly 的 `Clone()`
2. ✅ **引入 Async 模式**：`Arc<Collector>::run(urls)` 方法，取 LimitRule 中最大 parallelism 做信号量并发控制。返回 `Vec<(String, Result<Response>)>`
3. ✅ **引入 LimitRule**：添加 `add_limit(rule)` 方法，支持按域名通配符控制并发和延迟
4. ✅ **引入 URLFilters**：添加 `add_url_filter(regex)` / `add_disallowed_url_filter(regex)` 正则过滤
5. ✅ **引入 AllowedDomains**：添加域名白/黑名单 `set_allowed_domains` / `set_disallowed_domains`
6. ✅ **回调改为 Arc**：所有回调类型从 `Box<dyn Fn>` → `Arc<dyn Fn>`，使得 Collector 可 Clone，`visit` 签名从 `&mut self` 变为 `&self`
7. ✅ **并发安全的限速实现**：`MutexGuard` 在 `.await` 前释放，确保 `Send`

### 5.2 中期改进（优先级中）

6. **引入 Queue 调度器**：基于 channel 的工作队列，固定消费者并发
7. **添加 Request.Abort()**：在回调中中止请求处理
8. **添加回调解注册**：`on_html_detach`, `on_xml_detach`
9. **环境变量配置**：支持 `CRAWLKIT_*` 环境变量覆盖默认配置
10. **统一配置入口**：函数选项模式或 TypedBuilder

### 5.3 长期改进（优先级低）

11. **存储后端**：可替换的 visited 集合存储（Redis/Memcached）
12. **缓存层**：文件缓存或内存缓存
13. **Unmarshal**：声明式 HTML 提取（serde 反序列化）
14. **调试器接口**：结构化调试事件
15. **请求方法扩展**：HEAD/ PUT/ DELETE/ PATCH
16. **字符编码检测**：非 UTF-8 页面自动检测
