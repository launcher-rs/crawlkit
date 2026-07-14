//! # 回调模式示例（类 colly 风格）
//!
//! 演示 Collector 的完整回调链：
//! - `on_request`：请求前（可修改请求头、中止请求）
//! - `on_response`：收到 HTTP 响应后
//! - `on_html`：HTML 解析后，参数为 `HtmlContext { body, url }`
//! - `on_error`：请求失败或机器人验证
//!
//! 运行：`cargo run --example callback`

use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::reqwest();
    c.set_header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");

    // 请求前回调：打印即将访问的 URL
    c.on_request(|req| {
        println!("  [请求] {}", req.url);
    });

    // 响应回调：打印状态码
    c.on_response(|resp| {
        println!("  [响应] {} - 状态码: {}", resp.url, resp.status);
    });

    // HTML 回调：参数为 HtmlContext，包含 body 和 url
    c.on_html(|ctx| {
        let links = crawlkit::html::extract_links(ctx.body, "a[href]");
        let abs_links: Vec<String> = links
            .iter()
            .filter_map(|l| crawlkit::html::resolve_url(ctx.url, l))
            .collect();
        println!(
            "  [HTML] 在 {} 中发现 {} 个链接",
            ctx.url,
            abs_links.len()
        );
        for link in abs_links.iter().take(5) {
            println!("    -> {}", link);
        }
        if abs_links.len() > 5 {
            println!("    ... 还有 {} 个链接", abs_links.len() - 5);
        }
    });

    // 错误回调：请求失败或机器人验证
    c.on_error(|err| {
        eprintln!("  [错误] {}", err);
    });

    // 访问示例页面
    println!("=== 回调模式示例 ===\n");
    let _ = c.visit("https://news.ycombinator.com/").await;
}
