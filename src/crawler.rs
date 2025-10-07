pub mod downloader;
pub mod parser;
pub mod processor;
pub mod task;

pub use task::CrawlerTaskManager;

use anyhow::Result;
use reqwest::Client;
use scraper::Html;


use crate::epub::{Epub, EpubGenerator, Volume};
pub use downloader::ImageDownloader;
pub use parser::NovelParser;
pub use processor::ChapterProcessor;

// static MAX_RETRIES: u32 = 3;

pub struct DoclnCrawler {
    client: Client,
    base_url: String,
    parser: NovelParser,
    image_downloader: ImageDownloader,
}

impl DoclnCrawler {
    pub fn new() -> Self {
        // #[derive(Clone)]
        // pub struct Client {
        //     inner: Arc<ClientRef>
        // }
        // #[derive(Clone)]会为每一个字段调用Clone
        // Client的Clone只是增加inner的引用计数，并不会克隆底层数据
        // 所以这里直接克隆Client是安全且高效的
        // 而new一个新的Client会重新建立连接池，浪费资源
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap();

        // 定义重试策略：指数退避，最多重试3次
        // let retry_policy = ExponentialBackoff::builder().build_with_max_retries(MAX_RETRIES);
        // 创建重试中间件实例
        // let retry_middleware = RetryTransientMiddleware::new_with_policy(retry_policy);

        // #[derive(Clone, Default)]
        // pub struct ClientWithMiddleware {
        //     inner: reqwest::Client,                    // 核心 HTTP 客户端
        //     middleware_stack: Box<[Arc<dyn Middleware>]>,      // 中间件栈（请求处理）
        //     initialiser_stack: Box<[Arc<dyn RequestInitialiser>]>, // 初始化器栈（请求准备）
        // }
        //  middleware_stack 和 initialiser_stack 都是 Box 包裹的堆分配数组
        //  clone会分配堆内存并对每一个元素（即Arc）调用clone
        // ClientWithMiddleware的Clone开销较大，会涉及两次堆分配
        // 为了代码简洁，这里还是直接克隆ClientWithMiddleware
        // let client = ClientBuilder::new(client)
        //     .with(retry_middleware)
        //     .build();

        Self {
            client: client.clone(),
            base_url: "https://docln.net".to_string(),
            parser: NovelParser,
            image_downloader: ImageDownloader::new(client),
        }
    }

    pub async fn fetch_novel_info(&self, novel_id: u32) -> Result<Epub> {
        let url = format!("{}/sang-tac/{}", self.base_url, novel_id);

        println!("正在获取: {}", url);

        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;

        self.parse_novel_info(&html_content, &url, novel_id).await
    }

    pub async fn parse_novel_info(
        &self,
        html_content: &str,
        url: &str,
        novel_id: u32,
    ) -> Result<Epub> {
        let document = Html::parse_document(html_content);

        // 解析基本信息
        let mut epub = self.parser.parse_novel_info(html_content, url, novel_id)?;

        // 创建EPUB标准目录结构
        let epub_dir_name = format!("epub_{}", novel_id);
        let epub_dir = std::path::Path::new(&epub_dir_name);

        // 解析并下载封面图片
        if let Some(cover_url) = self.parser.extract_cover_url(&document) {
            match self
                .image_downloader
                .download_novel_cover(&cover_url, novel_id, &epub.title, epub_dir)
                .await
            {
                Ok(Some(path)) => epub.cover_image_path = Some(path),
                Ok(None) => println!("使用默认封面图片，跳过下载"),
                Err(e) => println!("下载封面图片失败: {}", e),
            }
        }

        // 解析卷信息
        let volume_infos = self.parser.parse_volume_info(&document);
        let mut volumes = Vec::new();

        for (volume_index, (volume_title, volume_id)) in volume_infos.into_iter().enumerate() {
            // 解析该卷的章节信息
            let mut chapters = self.parser.parse_volume_chapters(&document, &volume_id);

            // 查找卷封面图片
            let mut volume_cover_path = None;
            if let Some(cover_url) = self.parser.extract_volume_cover_url(&document, &volume_id) {
                match self
                    .image_downloader
                    .download_volume_cover_image(&cover_url, volume_index, &volume_title, epub_dir)
                    .await
                {
                    Ok(path) => volume_cover_path = path,
                    Err(e) => println!("下载卷 '{}' 封面图片失败: {}", volume_title, e),
                }
            }

            // 处理该卷的章节内容
            if !chapters.is_empty() {
                println!(
                    "\n正在处理卷 '{}' 的 {} 个章节...",
                    volume_title,
                    chapters.len()
                );

                // 创建EPUB标准的images目录
                let images_dir = epub_dir.join("OEBPS").join("images");
                std::fs::create_dir_all(&images_dir)?;

                let chapter_processor =
                    ChapterProcessor::new(self.client.clone(), self.base_url.clone());
                match chapter_processor.fetch_and_process_chapters(
                    &mut chapters,
                    volume_index,
                    images_dir,
                ) {
                    Ok(()) => println!("卷 '{}' 章节处理完成", volume_title),
                    Err(e) => println!("处理卷 '{}' 章节时出错: {}", volume_title, e),
                }
            }

            volumes.push(Volume {
                title: volume_title,
                volume_id,
                cover_image_path: volume_cover_path,
                chapters,
            });
        }

        epub.volumes = volumes;

        CrawlerTaskManager::wait_all_tasks().await;

        // 生成EPUB文件
        match EpubGenerator::new(epub.clone())
            .epub_dir(&epub_dir_name)
            .generate()
        {
            Ok(epub_filename) => {
                println!("EPUB文件生成成功: {}", epub_filename);
            }
            Err(e) => {
                println!("压缩EPUB文件失败: {}", e);
            }
        }

        Ok(epub)
    }

    pub async fn crawl_novel(&self, novel_id: u32) {
        match self.fetch_novel_info(novel_id).await {
            Ok(epub) => {
                println!("\n=== EPUB 信息 ===");
                println!("标题: {}", epub.title);
                println!("作者: {}", epub.author);
                if let Some(illustrator) = &epub.illustrator {
                    println!("插画师: {}", illustrator);
                }
                if !epub.summary.is_empty() {
                    println!("简介: {}", epub.summary);
                }
                if let Some(cover_path) = &epub.cover_image_path {
                    println!("封面: {}", cover_path);
                } else {
                    println!("封面: 使用默认封面");
                }
                println!("标签: {}", epub.tags.join(", "));

                // 显示卷信息
                if !epub.volumes.is_empty() {
                    println!("\n目录结构:");
                    for (i, volume) in epub.volumes.iter().enumerate() {
                        println!("  ├── {} (卷 {})", volume.title, i + 1);
                        if !volume.chapters.is_empty() {
                            let processed_count = volume
                                .chapters
                                .iter()
                                .filter(|c| c.xhtml_path.is_some())
                                .count();
                            if processed_count > 0 {
                                let display_count = std::cmp::min(3, processed_count);
                                let mut displayed = 0;
                                for chapter in &volume.chapters {
                                    if let Some(_) = &chapter.xhtml_path {
                                        if displayed < display_count {
                                            let chapter_prefix = if chapter.has_illustrations {
                                                "📄"
                                            } else {
                                                "📖"
                                            };
                                            println!(
                                                "  │   ├── {} {}",
                                                chapter_prefix, chapter.title
                                            );
                                            displayed += 1;
                                        }
                                    }
                                }
                                if processed_count > display_count {
                                    println!(
                                        "  │   └── ... (还有 {} 个章节)",
                                        processed_count - display_count
                                    );
                                }
                            }
                        }
                        if i < epub.volumes.len() - 1 {
                            println!("  │");
                        }
                    }
                }

                println!("URL: {}", epub.url);
                println!("==============\n");
            }
            Err(e) => {
                println!("爬取小说失败 (ID: {}): {}", novel_id, e);
            }
        }
    }
}
