# crawlkit-media

crawlkit 的媒体提取与下载模块。

从 HTML 文档中提取各类媒体资源：图片、视频、音频、文档、嵌入内容。同时提供可选的异步下载器，支持并发下载、重试和哈希验证。

## 提取器

```rust
use crawlkit_media::MediaExtractor;

let html = r#"
    <img src="hero.jpg" alt="封面">
    <img src="photo.png" srcset="photo-400.png 400w, photo-800.png 800w" loading="lazy">
    <video src="intro.mp4" poster="poster.jpg"></video>
    <iframe src="https://www.youtube.com/embed/abc123"></iframe>
    <a href="report.pdf">下载报告</a>
"#;

let extractor = MediaExtractor::new()
    .with_base_url("https://example.com");
let media = extractor.extract_all(html).unwrap();

println!("图片: {}", media.images.len());
println!("视频: {}", media.videos.len());
println!("音频: {}", media.audio.len());
println!("文档: {}", media.documents.len());
println!("嵌入: {}", media.embeds.len());
```

### 分类型提取

各媒体类型也提供独立的提取函数：

```rust
use crawlkit_media::{extract_images, extract_videos, extract_documents};

let html = "<html>...</html>";

let images = extract_images(html, None).unwrap();
let videos = extract_videos(html, None).unwrap();
let docs = extract_documents(html, None).unwrap();
```

### 快捷查询

```rust
use crawlkit_media::{has_images, has_videos, count_all_media, get_all_media_urls};

let html = "<html>...</html>";
assert!(has_images(html));
let total = count_all_media(html);
let urls = get_all_media_urls(html, "https://example.com");
```

## 下载器（需 `downloader` feature，默认启用）

```rust,no_run
use crawlkit_media::MediaDownloader;
use crawlkit_media::downloader::save_to_file;

let downloader = MediaDownloader::default();

// 单文件下载
let result = downloader.download("https://example.com/image.jpg").await.unwrap();
println!("类型: {:?}, 大小: {} 字节", result.media_type, result.size);

// 下载并保存到文件
save_to_file(&result.bytes, "downloads/image.jpg").await.unwrap();

// 批量下载
let urls = vec![
    "https://example.com/photo1.jpg",
    "https://example.com/photo2.jpg",
];
let results = downloader.download_all(&urls).await;
println!("成功: {}, 失败: {}", results.success.len(), results.failures.len());
```

### 下载配置

```rust,no_run
use crawlkit_media::{DownloadConfig, MediaDownloader};

let config = DownloadConfig {
    max_size: 10 * 1024 * 1024,   // 10MB
    timeout_secs: 30,
    retry_times: 3,
    concurrent: 4,
    ..DownloadConfig::default()
};

let downloader = MediaDownloader::with_config(config);
let result = downloader.download("https://example.com/video.mp4").await.unwrap();
```

### 哈希验证

```rust,no_run
use crawlkit_media::downloader::{download_with_hash, compute_sha256};

let data = download_with_hash(
    "https://example.com/file.zip",
    "sha256:e3b0c44298fc1c149afbf4c8996fb94...",
).await.unwrap();

let hash = compute_sha256(&data);
println!("SHA-256: {}", hash);
```

## 支持的媒体类型

| 类型 | 标签/属性 | 嵌入平台 |
|------|-----------|----------|
| 图片 | `<img>`, `<picture>`, `<figure>` | — |
| 视频 | `<video>`, `<source>` | YouTube, Vimeo, Dailymotion, Bilibili, Twitch |
| 音频 | `<audio>`, `<source>` | Spotify, SoundCloud, Apple Podcasts |
| 文档 | `<a href="*.pdf">`, Office 扩展名 | — |
| 嵌入 | `<iframe>`, `<embed>`, `<object>` | Google Maps, CodePen, Twitter, Instagram, Facebook, TikTok, Reddit |

## Feature flags

| Feature | 默认 | 说明 |
|---------|------|------|
| `downloader` | 启用 | 异步下载器（reqwest + tokio + sha2） |
