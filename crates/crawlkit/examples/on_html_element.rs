use crawlkit::Collector;

#[tokio::main]
async fn main() {
    let mut c = Collector::new();

    c.on_request(|req| {
        println!("即将请求: {}", req.url);
        println!("即将header: {:?}", req.headers);
    });

    c.on_response(|resp| {
        println!("  [响应] {} - 状态码: {}", resp.url, resp.status);
    });

    c.on_html_element("a", |element| {
        let href = element.absolute_url("href").unwrap_or_default();
        println!("element : {}, text: {}", href, element.text());
    });

    c.visit("http://go-colly.org/").await.unwrap();
}
