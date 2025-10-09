pub mod downloader;
pub mod parser;
pub mod processor;
pub mod task;

pub use task::CrawlerTaskManager;

use std::path::PathBuf;

use anyhow::Result;
use tokio::fs;

pub use downloader::Downloader;
pub use parser::Parser;
pub use processor::Processor;

// static MAX_RETRIES: u32 = 3;

pub struct DoclnCrawler {
    parser: Parser,
    downloader: Downloader,
}

impl DoclnCrawler {
    pub fn new() -> Self {
        Self {
            parser: Parser,
            downloader: Downloader::new("https://docln.net".to_owned()),
        }
    }

    pub async fn generate_epub(&self, novel_id: String) -> Result<()> {
        let epub_name = format!("docln_{}", novel_id);
        let epub_dir = PathBuf::from(&epub_name);
        let meta_dir = epub_dir.join("META-INF");
        let oebps_dir = epub_dir.join("OEBPS");
        let image_dir = oebps_dir.join("images");
        let text_dir = oebps_dir.join("text");

        fs::create_dir(&epub_dir).await?;
        fs::create_dir(&meta_dir).await?;
        fs::create_dir(&oebps_dir).await?;
        fs::create_dir(&image_dir).await?;
        fs::create_dir(&text_dir).await?;

        let processor = Processor::new(image_dir.clone(), text_dir.clone());
        let novel_html = self.downloader.novel_info(&novel_id).await?;
        let mut epub = self.parser.novel_info(&novel_html, novel_id)?;
        if let Some(cover_url) = epub.cover {
            let (cover_bytes, extension) = self.downloader.image(&cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            epub.cover = Some(cover_name);
        }

        for volume in epub.volumes.iter_mut() {
            if let Some(volume_cover_url) = &volume.cover {
                let (cover_bytes, extension) = self.downloader.image(&volume_cover_url).await?;
                let cover_name = processor.write_image(cover_bytes, extension).await?;
                volume.cover = Some(cover_name);
            }

            let cover_html = volume.cover_html();
            processor
                .write_html(cover_html, &volume.cover_chapter)
                .await?;

            for chapter in volume.chapters.iter_mut() {
                let chapter_html = self.downloader.chapter(&chapter.url).await?;
                let mut content = self.parser.chapter_content(chapter_html)?;
                if chapter.has_illustrations {
                    let srcs = self.parser.chapter_srcs(&content);
                    for src in srcs {
                        let Ok((image_bytes, extension)) = self.downloader.image(&src).await else {
                            println!("图片下载失败: {}", src);
                            continue;
                        };

                        let Ok(image_name) = processor.write_image(image_bytes, extension).await else {
                            println!("图片保存失败: {}", src);
                            continue;
                        };

                            content = content.replace(&src, &format!("../images/{}", image_name));
                            chapter.images.push(image_name);
                    }
                }
                processor.write_chapter(content, chapter).await?;
            }
        }

        epub.epub_dir = epub_dir;
        epub.meta_dir = meta_dir;
        epub.oebps_dir = oebps_dir;
        epub.image_dir = image_dir;
        epub.text_dir = text_dir;

        epub.generate().await?;

        Ok(())
    }
}
