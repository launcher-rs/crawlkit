//! 示例 3：媒体提取与下载
//!
//! 演示如何从 HTML 中提取图片、视频、音频、文档和嵌入内容，
//! 以及如何使用下载器下载媒体文件。
//!
//! 运行：`cargo run --example extract_media`

use crawlkit::media::{
    MediaExtractor, MediaExtractorBuilder, MediaType, MediaDownloader, DownloadConfig,
    filter_placeholders, get_best_image_url,
    count_all_media, has_any_media,
};

fn main() {
    // 模拟一篇包含多种媒体的文章页 HTML
    let html = r#"
    <html>
    <head>
        <title>媒体示例文章</title>
    </head>
    <body>
        <article>
            <h1>2026 年最佳摄影器材</h1>
            <p>以下是本周推荐的相机和镜头：</p>

            <!-- 图片：标准 + srcset + 懒加载 -->
            <img src="https://example.com/photos/camera.jpg"
                 alt="旗舰相机"
                 width="1200" height="800">
            <img src="https://example.com/photos/lens.jpg"
                 srcset="https://example.com/photos/lens-small.jpg 400w,
                         https://example.com/photos/lens-large.jpg 1200w"
                 alt="专业镜头">
            <img data-src="https://example.com/photos/lazy-photo.jpg"
                 class="lazy" alt="延迟加载图片">

            <!-- 视频：HTML5 + 嵌入平台 -->
            <video controls width="720">
                <source src="https://example.com/videos/review.mp4" type="video/mp4">
                <source src="https://example.com/videos/review.webm" type="video/webm">
                <track kind="subtitles" src="subtitles_zh.vtt" srclang="zh" label="中文">
            </video>
            <iframe src="https://www.youtube.com/embed/dQw4w9WgXcQ"
                    width="560" height="315"
                    title="YouTube 视频"></iframe>

            <!-- 音频：HTML5 + 流媒体 -->
            <audio controls>
                <source src="https://example.com/audio/podcast.mp3" type="audio/mpeg">
                <source src="https://example.com/audio/podcast.ogg" type="audio/ogg">
            </audio>
            <iframe src="https://open.spotify.com/embed/track/12345"
                    width="300" height="80"></iframe>

            <!-- 文档链接 -->
            <a href="https://example.com/docs/specification.pdf">产品规格书 (PDF)</a>
            <a href="https://example.com/docs/review.docx">评测报告 (Word)</a>
            <a href="https://example.com/docs/data.xlsx">测试数据 (Excel)</a>

            <!-- 嵌入内容 -->
            <iframe src="https://www.google.com/maps/embed?pb=!1m18"
                    width="600" height="450"
                    allowfullscreen></iframe>
            <blockquote class="twitter-tweet">
                <a href="https://twitter.com/user/status/123456789"></a>
            </blockquote>

            <!-- 带 download 属性的下载链接 -->
            <a href="https://example.com/downloads/firmware_v2.zip" download>固件下载</a>
        </article>
    </body>
    </html>
    "#;

    println!("═══ 媒体提取示例 ═══\n");

    // ------------------------------------------------------------------
    // 1. 基础使用：提取所有媒体
    // ------------------------------------------------------------------
    println!("── 1. 提取所有媒体 ──");
    let extractor = MediaExtractor::new()
        .with_base_url("https://example.com");

    let media = extractor.extract_all(html).unwrap();

    println!("  图片: {} 张", media.images.len());
    for img in &media.images {
        println!("    · {} ({}x{})",
            img.absolute_url.as_deref().unwrap_or(&img.src),
            img.width.unwrap_or(0),
            img.height.unwrap_or(0),
        );
    }

    println!("  视频: {} 个", media.videos.len());
    for video in &media.videos {
        println!("    · {} [来源: {:?}]",
            video.absolute_url.as_deref().unwrap_or("(嵌入)"),
            video.platform,
        );
    }

    println!("  音频: {} 个", media.audio.len());
    for audio in &media.audio {
        println!("    · {} [来源: {:?}]",
            audio.absolute_url.as_deref().unwrap_or("(嵌入)"),
            audio.platform,
        );
    }

    println!("  文档: {} 个", media.documents.len());
    for doc in &media.documents {
        println!("    · {} [类型: {:?}]", doc.url, doc.doc_type);
    }

    println!("  嵌入: {} 个", media.embeds.len());
    for emb in &media.embeds {
        println!("    · {} [平台: {:?}]", emb.url, emb.platform);
    }

    // ------------------------------------------------------------------
    // 2. 统计信息
    // ------------------------------------------------------------------
    println!("\n── 2. 媒体统计 ──");
    let counts = count_all_media(html);
    println!("  合计: {} 个媒体资源", counts.total);
    println!("  包含媒体? {}", if has_any_media(html) { "是" } else { "否" });

    // ------------------------------------------------------------------
    // 3. 筛选最佳图片
    // ------------------------------------------------------------------
    println!("\n── 3. 筛选最佳图片 ──");
    let all_images = extractor.extract_images(html).unwrap();
    let filtered = filter_placeholders(all_images);
    println!("  移除占位图后: {} 张（原 {} 张）", filtered.len(), media.images.len());

    if let Some(best_img) = filtered.first() {
        println!("  最佳图片 URL: {}", get_best_image_url(best_img));
    }

    // ------------------------------------------------------------------
    // 4. 按类型提取
    // ------------------------------------------------------------------
    println!("\n── 4. 按类型提取 URL ──");
    let image_urls = extractor.get_urls_by_type(html, MediaType::Image);
    println!("  图片 URL: {}", image_urls.join(", "));

    let doc_urls = extractor.get_urls_by_type(html, MediaType::Document);
    println!("  文档 URL: {}", doc_urls.join(", "));

    // ------------------------------------------------------------------
    // 5. 使用 Builder
    // ------------------------------------------------------------------
    println!("\n── 5. 使用 Builder 配置 ──");
    let custom = MediaExtractorBuilder::new()
        .extract_images(true)
        .extract_videos(true)
        .extract_audio(false)   // 跳过音频
        .extract_documents(true)
        .extract_embeds(true)
        .filter_placeholders(true)
        .min_image_size(200, 150)
        .base_url("https://example.com")
        .build();

    let custom_media = custom.extract_all(html).unwrap();
    println!("  Builder 提取结果:");
    println!("    图片: {}（已过滤占位图 + 最小尺寸）", custom_media.images.len());
    println!("    视频: {}", custom_media.videos.len());
    println!("    文档: {}", custom_media.documents.len());
    println!("    嵌入: {}", custom_media.embeds.len());

    // ------------------------------------------------------------------
    // 6. 下载器示例（演示配置，不实际执行）
    // ------------------------------------------------------------------
    println!("\n── 6. 下载器配置 ──");
    let dl_config = DownloadConfig {
        max_concurrent: 4,
        max_retries: 3,
        retry_delay_ms: 1000,
        timeout_secs: 30,
        max_file_size: Some(10 * 1024 * 1024),
        encode_base64: false,
        user_agent: "crawlkit-media/0.1.0".to_string(),
    };
    let _downloader = MediaDownloader::new(dl_config.clone());
    println!("  下载器就绪:");
    println!("    并发: {}", dl_config.max_concurrent);
    println!("    重试: {} 次", dl_config.max_retries);
    println!("    超时: {} 秒", dl_config.timeout_secs);
    println!("    限大: {:?}", dl_config.max_file_size);
    println!();
    println!("  要实际下载媒体文件，请调用:");
    println!("    downloader.download(url).await  — 下载返回字节");
    println!("    downloader.download_to_file(url, path).await  — 保存到文件");
    println!("    downloader.download_many(&urls).await  — 批量并发下载");

    // ------------------------------------------------------------------
    // 7. 实用辅助函数
    // ------------------------------------------------------------------
    println!("\n── 7. 辅助函数示例 ──");
    let sample_urls = [
        "https://example.com/image.jpg",
        "https://example.com/video.mp4",
        "https://example.com/doc.pdf",
        "data:image/png;base64,iVBORw0KGgo=",
    ];
    for url in &sample_urls {
        println!("  {} → 可下载: {}", url, crawlkit::media::is_downloadable(url));
    }
    println!("  URL 转文件名: {}",
        crawlkit::media::url_to_filename("https://example.com/photos/sunset.jpg"));

    println!("\n═══ 示例结束 ═══");
}
