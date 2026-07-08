//! # 示例：媒体提取（使用本地测试页面）
//!
//! 演示 MediaExtractor 的各种用法，使用 assets/media_test.html 作为测试数据。
//!
//! 运行：`cargo run --example extract_media_advanced`

use std::path::Path;

use crawlkit::media::{
    ExtractedMedia, MediaExtractor, MediaType,
    get_image_urls, get_video_urls,
};

/// 读取测试 HTML 文件
fn load_test_html() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("crawlkit-media/assets/media_test.html");
    std::fs::read_to_string(&path).expect("无法读取测试页面")
}

fn main() {
    let html = load_test_html();

    // ── 方式 1：统一提取 ──
    println!("=== MediaExtractor 统一提取 ===");
    let extractor = MediaExtractor::new()
        .with_base_url("https://example.com");
    let media = extractor.extract_all(&html).unwrap();
    print_summary(&media);

    // ── 方式 2：MediaExtractor 分类型提取 ──
    println!("\n=== MediaExtractor 分类型提取 ===");
    let images = extractor.extract_images(&html).unwrap();
    println!("图片: {} 个", images.len());
    for img in &images {
        println!("  - {} ({}x{})", img.src, img.width.unwrap_or(0), img.height.unwrap_or(0));
    }

    let videos = extractor.extract_videos(&html).unwrap();
    println!("视频: {} 个", videos.len());
    for v in &videos {
        println!("  - {} [{:?}]", v.src, v.platform);
    }

    let audio = extractor.extract_audio(&html).unwrap();
    println!("音频: {} 个", audio.len());

    let docs = extractor.extract_documents(&html).unwrap();
    println!("文档: {} 个", docs.len());
    for d in &docs {
        println!("  - {} ({:?})", d.url, d.doc_type);
    }

    let embeds = extractor.extract_embeds(&html).unwrap();
    println!("嵌入: {} 个", embeds.len());
    for e in &embeds {
        println!("  - {} ({:?})", e.url, e.platform);
    }

    // ── 方式 3：快捷 URL 收集 ──
    println!("\n=== 快捷 URL 收集 ===");
    let base = Some("https://example.com");
    println!("图片 URL: {:?}", get_image_urls(&html, base));
    println!("视频 URL: {:?}", get_video_urls(&html, base));

    // ── 方式 4：快捷查询 ──
    println!("\n=== 快捷查询 ===");
    println!("是否包含图片: {}", extractor.has_media_type(&html, MediaType::Image));
    println!("是否包含视频: {}", extractor.has_media_type(&html, MediaType::Video));
    println!("是否包含任何媒体: {}", extractor.has_media(&html));
    println!("提取结果中是否有媒体: {}", media.has_media());
    println!("媒体总数: {}", media.total_count());
}

fn print_summary(media: &ExtractedMedia) {
    println!("图片: {} 个", media.images.len());
    println!("视频: {} 个", media.videos.len());
    println!("音频: {} 个", media.audio.len());
    println!("文档: {} 个", media.documents.len());
    println!("嵌入: {} 个", media.embeds.len());
    println!("总计: {} 个", media.total_count());
}
