use crawlkit::Collector;

#[tokio::main]
async fn main() {
    crawlkit::log::init();

    let mut c = Collector::new();

    c.on_request(|req| {
        println!("即将请求: {}", req.url);
    });

    c.on_response(|resp| {
        println!("  [响应] {} - 状态码: {}", resp.url, resp.status);
    });

    // CSS 选择器方式 — 用 on_html_element
    println!("=== on_html_element(\"a\") ===");
    c.on_html_element("a", |e| {
        println!(
            "  href: {}, text: {}",
            e.absolute_url("href").unwrap_or_default(),
            e.text()
        );
    });

    // XPath 方式 — 用 on_xml_element
    println!("=== on_xml_element(\"//a\") ===");
    c.on_xml_element("//a", |e| {
        println!(
            "  href: {}, text: {}",
            e.absolute_url("href").unwrap_or_default(),
            e.text()
        );
    });

    c.visit("http://go-colly.org/").await.unwrap();
}
