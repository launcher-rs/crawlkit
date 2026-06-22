//! # 示例 1：回调模式（类 colly 风格）
//!
//! 演示如何使用 Collector 的 on_request / on_response / on_html 回调链。
//!
//! 运行：`cargo run --example callback`

use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::new();
    c.set_header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");

    // 请求前回调：打印即将访问的 URL
    c.on_request(|req| {
        println!("  [请求] {}", req.url);
    });

    // 响应回调：打印状态码
    c.on_response(|resp| {
        println!("  [响应] {} - 状态码: {}", resp.url, resp.status);
    });

    // HTML 回调：提取链接
    c.on_html(|html, base_url| {
        let links = crawlkit::html::extract_links(html, "a[href]");
        let abs_links: Vec<String> = links
            .iter()
            .filter_map(|l| crawlkit::html::resolve_url(base_url, l))
            .collect();
        println!("  [HTML] 在 {} 中发现 {} 个链接", base_url, abs_links.len());
        for link in abs_links.iter().take(5) {
            println!("    -> {}", link);
        }
        if abs_links.len() > 5 {
            println!("    ... 还有 {} 个链接", abs_links.len() - 5);
        }
    });

    // 访问示例页面
    let _ = c.visit("https://news.ycombinator.com/").await;
}
