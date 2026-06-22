# crawlkit-parser

HTML 解析与内容提取模块。基于 `scraper`（html5ever）和 `dom_smoothie`（Readability 模式）。

## 功能

| 函数 | 说明 |
|------|------|
| `extract_links` | CSS 选择器提取链接 |
| `extract_article` | 启发式提取文章（标题/正文/日期/作者） |
| `extract_readable_content` | Readability 模式提取正文 |
| `extract_content_by_selector` | CSS 选择器提取正文 |
| `extract_content` | 优先 Readability，失败回退到 CSS |
| `extract_texts` | 提取匹配选择器的所有文本 |
| `extract_attributes` | 提取匹配选择器元素的属性值 |
| `resolve_url` | 相对 URL 转绝对 URL |

## 使用

```rust
use crawlkit_parser::html;

let html = r#"<html><body><article><p>正文</p></article></body></html>"#;

// 提取链接
let links = html::extract_links(html, "a[href]");

// 可读性提取
let content = html::extract_readable_content(html).unwrap();

// CSS 选择器提取
let content = html::extract_content_by_selector(html, "article").unwrap();
```
