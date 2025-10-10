pub mod downloader;
pub mod parser;
pub mod processor;
pub mod task;

use std::mem::take;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::fs;
use tracing::{error, info, instrument};

use crate::epub::{Chapter, Epub, Volume};
use downloader::Downloader;
use parser::Parser;
pub use task::TaskManager;

type Processor = Arc<processor::Processor>;
type ChapterTaskManager = TaskManager<Chapter>;
type VolumeTaskManager = TaskManager<(Volume, ChapterTaskManager)>;
type EpubTaskManager = TaskManager<(Epub, VolumeTaskManager)>;

// static MAX_RETRIES: u32 = 3;

pub struct DoclnCrawler {
    parser: Parser,
    downloader: Downloader,
}

impl Default for DoclnCrawler {
    fn default() -> Self {
        Self::new()
    }
}

impl DoclnCrawler {
    pub fn new() -> Self {
        Self {
            parser: Parser,
            downloader: Downloader::new("https://docln.net".to_owned()),
        }
    }

    pub async fn crawl(&self, ids: Vec<String>) -> Result<()> {
        let mut epub_tasks = Self::epub_tasks(ids, &self.downloader, &self.parser);
        let results = epub_tasks.wait().await?;
        for (mut epub, mut volume_tasks) in results {
            Self::set_epub_volumes(&mut epub, &mut volume_tasks).await?;
            let _ = epub.generate().await?;
        }
        Ok(())
    }
}

impl DoclnCrawler {
        #[instrument(skip_all)]
    async fn set_epub_volumes(epub: &mut Epub, volume_tasks: &mut VolumeTaskManager) -> Result<()> {
        info!("正在整合小说的卷信息");
        let results = volume_tasks.wait().await?;
        for (mut volume, mut chapter_tasks) in results {
            info!("正在整合第 {} 卷", volume.index + 1);
            let chapters = chapter_tasks.wait().await?;
            volume.chapters = chapters;
            info!("正在排序第 {} 卷的章节", volume.index + 1);
            volume.chapters.sort_by_key(|c| c.index);
            info!("完成整合第 {} 卷", volume.index + 1);
            epub.volumes.push(volume);
        }
        info!("正在排序小说的卷信息");
        epub.volumes.sort_by_key(|v| v.index);
        info!("完成整合小说的卷信息");
        Ok(())
    }

    fn epub_tasks(ids: Vec<String>, downloader: &Downloader, parser: &Parser) -> EpubTaskManager {
        let mut task_manager = TaskManager::new();
        for id in ids {
            let downloader = downloader.clone();
            let parser = parser.clone();

            let epub_future = Self::epub_task(id, downloader, parser);
            task_manager.spawn(epub_future);
        }
        task_manager
    }

    fn volume_tasks(
        volumes: Vec<Volume>,
        processor: &Processor,
        downloader: &Downloader,
        parser: &Parser,
    ) -> VolumeTaskManager {
        let mut task_manager = TaskManager::new();
        for volume in volumes {
            let processor = processor.clone();
            let downloader = downloader.clone();
            let parser = parser.clone();

            let volume_future = Self::volume_task(volume, processor, downloader, parser);
            task_manager.spawn(volume_future);
        }
        task_manager
    }

    fn chapter_tasks(
        chapters: Vec<Chapter>,
        processor: &Processor,
        downloader: &Downloader,
        parser: &Parser,
    ) -> ChapterTaskManager {
        let mut task_manager = TaskManager::new();
        for chapter in chapters {
            let downloader = downloader.clone();
            let parser = parser.clone();
            let processor = processor.clone();
            let chapter_future = Self::chapter_task(chapter, processor, downloader, parser);
            task_manager.spawn(chapter_future);
        }
        task_manager
    }

    #[instrument(skip_all)]
    pub async fn epub_task(
        novel_id: String,
        downloader: Downloader,
        parser: Parser,
    ) -> Result<(Epub, VolumeTaskManager)> {
        info!("正在爬取 ID为 {} 的小说...", novel_id);
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

        let processor = Arc::new(processor::Processor::new(
            image_dir.clone(),
            text_dir.clone(),
        ));
        let novel_html = downloader.novel_info(&novel_id).await?;
        let mut epub = parser.novel_info(&novel_html, novel_id)?;
        if let Some(cover_url) = epub.cover {
            let (cover_bytes, extension) = downloader.image(&cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            epub.cover = Some(cover_name);
        }

        let volume_tasks =
            Self::volume_tasks(take(&mut epub.volumes), &processor, &downloader, &parser);

        epub.epub_dir = epub_dir;
        epub.meta_dir = meta_dir;
        epub.oebps_dir = oebps_dir;
        epub.image_dir = image_dir;
        epub.text_dir = text_dir;

        info!("完成爬取 ID为 {} 的小说", epub.id);
        Ok((epub, volume_tasks))
    }

    #[instrument(skip_all)]
    async fn volume_task(
        mut volume: Volume,
        processor: Processor,
        downloader: Downloader,
        parser: Parser,
    ) -> Result<(Volume, ChapterTaskManager)> {
        info!("正在处理第 {} 卷", volume.index + 1);
        if let Some(volume_cover_url) = &volume.cover {
            let (cover_bytes, extension) = downloader.image(volume_cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            volume.cover = Some(cover_name);
        }

        let cover_html = volume.cover_html();
        processor
            .write_html(cover_html, &volume.cover_chapter)
            .await?;
        let chapter_tasks =
            Self::chapter_tasks(take(&mut volume.chapters), &processor, &downloader, &parser);
        info!("完成处理第 {} 卷", volume.index + 1);
        Ok((volume, chapter_tasks))
    }

    #[instrument(skip_all)]
    async fn chapter_task(
        mut chapter: Chapter,
        processor: Processor,
        downloader: Downloader,
        parser: Parser,
    ) -> Result<Chapter> {
        info!("正在处理第 {} 章: {}", chapter.index + 1, chapter.title);
        let chapter_html = downloader.chapter(&chapter.url).await?;
        let mut content = parser.chapter_content(chapter_html)?;
        if chapter.has_illustrations {
            let srcs = parser.chapter_srcs(&content);
            for src in srcs {
                let Ok((image_bytes, extension)) = downloader.image(&src).await else {
                    error!("图片下载失败: {}", src);
                    continue;
                };

                let Ok(image_name) = processor.write_image(image_bytes, extension).await else {
                    error!("图片保存失败: {}", src);
                    continue;
                };

                content = content.replace(&src, &format!("../images/{}", image_name));
                chapter.images.push(image_name);
            }
        }
        processor.write_chapter(content, &chapter).await?;
        info!("完成处理第 {} 章: {}", chapter.index + 1, chapter.title);
        Ok(chapter)
    }
}
