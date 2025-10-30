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

use crate::{
    config::get_site_config,
    epub::{self, Chapter, Epub, VolOrChap, Volume},
};
use downloader::Downloader;
use parser::Parser;
pub use task::TaskManager;

type Processor = Arc<processor::Processor>;
type ChapterTaskManager = TaskManager<Chapter>;
type VolumeTaskManager = TaskManager<(Volume, ChapterTaskManager)>;

// static MAX_RETRIES: u32 = 3;

pub struct DoclnCrawler {
    parser: Parser,
    downloader: Downloader,
}

impl DoclnCrawler {
    pub fn new(url: String, site_name: &str) -> Self {
        Self {
            parser: Parser::new(site_name),
            downloader: Downloader::new(site_name, url),
        }
    }

    pub async fn crawl(&self, id: String, site_name: String) -> Result<()> {
        let id = format!("{}_{}", site_name, id);

        let content_extractor = &get_site_config(site_name.as_str())?
            .get_chapter_config()
            .expect("没有章节配置")
            .content;

        if let Some(_) = &content_extractor.next_url {
            let epub =
                Self::epub_sequential(id, self.downloader.clone(), self.parser.clone()).await?;
            let _ = epub.generate().await?;
        } else {
            let (mut epub, children_tasks) =
                Self::epub_task(id, self.downloader.clone(), self.parser.clone()).await?;

            Self::set_epub_children(&mut epub, children_tasks).await?;
            let _ = epub.generate().await?;
        }

        Ok(())
    }
}

impl DoclnCrawler {
    async fn set_epub_children(epub: &mut Epub, children_tasks: VolOrChapTasks) -> Result<()> {
        match children_tasks {
            VolOrChapTasks::Volume(volume_tasks) => {
                let volumes = Self::sort_volumes(volume_tasks).await?;
                epub.children = epub::VolOrChap::Volumes(volumes);
            }
            VolOrChapTasks::Chapter(chapter_tasks) => {
                let chapters = Self::sort_chapters(chapter_tasks).await?;
                epub.children = epub::VolOrChap::Chapters(chapters);
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn sort_volumes(mut volume_tasks: VolumeTaskManager) -> Result<Vec<Volume>> {
        let mut volumes = Vec::new();
        info!("正在整合小说的卷信息");
        let results = volume_tasks.wait().await?;
        for (mut volume, chapter_tasks) in results {
            info!("正在整合第 {} 卷", volume.index);
            let chapters = Self::sort_chapters(chapter_tasks).await?;
            volume.chapters = chapters;
            info!("完成整合第 {} 卷", volume.index);
            volumes.push(volume);
        }
        info!("正在排序小说的卷信息");
        volumes.sort_by_key(|v| v.index);
        info!("完成整合小说的卷信息");
        Ok(volumes)
    }

    async fn sort_chapters(mut chapter_tasks: ChapterTaskManager) -> Result<Vec<Chapter>> {
        let mut chapters = chapter_tasks.wait().await?;
        chapters.sort_by_key(|c| c.index);
        Ok(chapters)
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

            let volume_future = Self::volume_task(volume, processor, downloader, *parser);
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
            let processor = processor.clone();
            let chapter_future = Self::chapter_task(chapter, processor, downloader, *parser);
            task_manager.spawn(chapter_future);
        }
        task_manager
    }

    #[instrument(skip_all)]
    pub async fn epub_task(
        novel_id: String,
        mut downloader: Downloader,
        parser: Parser,
    ) -> Result<(Epub, VolOrChapTasks)> {
        info!("正在爬取 ID为 {} 的小说...", novel_id);
        let epub_name = format!("{}", novel_id);
        let epub_dir = PathBuf::from(&epub_name);
        let meta_dir = epub_dir.join("META-INF");
        let oebps_dir = epub_dir.join("OEBPS");
        let image_dir = oebps_dir.join("Images");
        let text_dir = oebps_dir.join("Text");

        fs::create_dir(&epub_dir).await?;
        fs::create_dir(&meta_dir).await?;
        fs::create_dir(&oebps_dir).await?;
        fs::create_dir(&image_dir).await?;
        fs::create_dir(&text_dir).await?;

        let processor = Arc::new(processor::Processor::new(
            image_dir.clone(),
            text_dir.clone(),
        ));
        let novel_html = downloader.novel_info().await?;
        let mut epub = parser.novel_info(&novel_html, novel_id)?;
        if let Some(cover_url) = take(&mut epub.cover) {
            let (cover_bytes, extension) = downloader.image(&cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            epub.cover = Some(cover_name);
        }

        let children_tasks =
            match take(&mut epub.children) {
                epub::VolOrChap::Volumes(volumes) => VolOrChapTasks::Volume(Self::volume_tasks(
                    volumes,
                    &processor,
                    &downloader,
                    &parser,
                )),
                epub::VolOrChap::Chapters(chapters) => VolOrChapTasks::Chapter(
                    Self::chapter_tasks(chapters, &processor, &downloader, &parser),
                ),
            };

        epub.epub_dir = epub_dir;
        epub.meta_dir = meta_dir;
        epub.oebps_dir = oebps_dir;
        epub.image_dir = image_dir;
        epub.text_dir = text_dir;

        info!("完成爬取 ID为 {} 的小说", epub.id);
        Ok((epub, children_tasks))
    }

    #[instrument(skip_all)]
    async fn volume_task(
        mut volume: Volume,
        processor: Processor,
        mut downloader: Downloader,
        parser: Parser,
    ) -> Result<(Volume, ChapterTaskManager)> {
        info!("正在处理第 {} 卷", volume.index);
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
        info!("完成处理第 {} 卷", volume.index);
        Ok((volume, chapter_tasks))
    }

    #[instrument(skip_all)]
    async fn chapter_task(
        mut chapter: Chapter,
        processor: Processor,
        mut downloader: Downloader,
        parser: Parser,
    ) -> Result<Chapter> {
        info!("正在处理第 {} 章: {}", chapter.index, chapter.title);
        let chapter_html = downloader.chapter(&chapter.url).await?;
        let mut content = parser.chapter_content(chapter_html)?;
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

            content = content.replace(&src, &format!("../Images/{}", image_name));
            chapter.images.push(image_name);
        }
        processor.write_chapter(content, &chapter).await?;
        info!("完成处理第 {} 章: {}", chapter.index, chapter.title);
        Ok(chapter)
    }
}

impl DoclnCrawler {
    async fn volume_sequential(
        mut volumes: Vec<Volume>,
        processor: &Processor,
        downloader: &mut Downloader,
        parser: &Parser,
    ) -> Result<Vec<Volume>> {
        let mut next_url = volumes.first().unwrap().chapters.first().map(|c| c.url.clone()).unwrap();
        for volume in volumes.iter_mut() {
            info!("正在处理第 {} 卷", volume.index);
            if let Some(volume_cover_url) = &volume.cover {
                let (cover_bytes, extension) = downloader.image(volume_cover_url).await?;
                let cover_name = processor.write_image(cover_bytes, extension).await?;
                volume.cover = Some(cover_name);
            }

            let cover_html = volume.cover_html();
            processor
                .write_html(cover_html, &volume.cover_chapter)
                .await?;
            let chapters = Self::chapters_sequential(
                take(&mut volume.chapters),
                processor,
                downloader,
                parser,
                &mut next_url,
            )
            .await?;
            volume.chapters = chapters;
            info!("完成处理第 {} 卷", volume.index);
        }
        Ok(volumes)
    }

    // todo: 为什么要返回chapter
    async fn chapters_sequential(
        mut chapters: Vec<Chapter>,
        processor: &Processor,
        downloader: &Downloader,
        parser: &Parser,
        next_url: &mut String,
    ) -> Result<Vec<Chapter>> {
        let mut downloader = downloader.clone();
        let chapter_contents = downloader.chapters_sequential(&chapters, next_url).await?;
        for (chapter, mut content) in chapters.iter_mut().zip(chapter_contents) {
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

                content = content.replace(&src, &format!("../Images/{}", image_name));
                chapter.images.push(image_name);
            }
            processor.write_chapter(content, chapter).await.expect("");
        }
        Ok(chapters)
    }

    #[instrument(skip_all)]
    pub async fn epub_sequential(
        novel_id: String,
        mut downloader: Downloader,
        parser: Parser,
    ) -> Result<Epub> {
        info!("正在爬取 ID为 {} 的小说...", novel_id);
        let epub_name = format!("{}", novel_id);
        let epub_dir = PathBuf::from(&epub_name);
        let meta_dir = epub_dir.join("META-INF");
        let oebps_dir = epub_dir.join("OEBPS");
        let image_dir = oebps_dir.join("Images");
        let text_dir = oebps_dir.join("Text");

        fs::create_dir(&epub_dir).await?;
        fs::create_dir(&meta_dir).await?;
        fs::create_dir(&oebps_dir).await?;
        fs::create_dir(&image_dir).await?;
        fs::create_dir(&text_dir).await?;

        let processor = Arc::new(processor::Processor::new(
            image_dir.clone(),
            text_dir.clone(),
        ));
        let novel_html = downloader.novel_info().await?;
        let mut epub = parser.novel_info(&novel_html, novel_id)?;
        if let Some(cover_url) = take(&mut epub.cover) {
            let (cover_bytes, extension) = downloader.image(&cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            epub.cover = Some(cover_name);
        }

        let children = match take(&mut epub.children) {
            epub::VolOrChap::Volumes(volumes) => VolOrChap::Volumes(
                Self::volume_sequential(volumes, &processor, &mut downloader, &parser).await?,
            ),
            epub::VolOrChap::Chapters(chapters) => {
                let mut next_url = chapters.first().map(|c| c.url.clone()).unwrap();
                VolOrChap::Chapters(
                Self::chapters_sequential(chapters, &processor, &downloader, &parser, &mut next_url).await?
            )
            }
        };

        epub.children = children;
        epub.epub_dir = epub_dir;
        epub.meta_dir = meta_dir;
        epub.oebps_dir = oebps_dir;
        epub.image_dir = image_dir;
        epub.text_dir = text_dir;

        info!("完成爬取 ID为 {} 的小说", epub.id);
        Ok(epub)
    }
}

pub enum VolOrChapTasks {
    Volume(VolumeTaskManager),
    Chapter(ChapterTaskManager),
}
