pub mod downloader;
pub mod parser;
pub mod processor;
pub mod task;

use std::mem::take;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::fs;

use crate::epub::{Chapter, Epub, Volume};
use downloader::Downloader;
use parser::Parser;
pub use task::TaskManager;

type Processor = Arc<processor::Processor>;

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

        let processor = Arc::new(processor::Processor::new(
            image_dir.clone(),
            text_dir.clone(),
        ));
        let novel_html = self.downloader.novel_info(&novel_id).await?;
        let mut epub = self.parser.novel_info(&novel_html, novel_id)?;
        if let Some(cover_url) = epub.cover {
            let (cover_bytes, extension) = self.downloader.image(&cover_url).await?;
            let cover_name = processor.write_image(cover_bytes, extension).await?;
            epub.cover = Some(cover_name);
        }

        let mut task_manager = Self::volume_tasks(
            take(&mut epub.volumes),
            &processor,
            &self.downloader,
            &self.parser,
        );

        Self::set_epub_volumes(&mut epub, &mut task_manager).await?;

        epub.epub_dir = epub_dir;
        epub.meta_dir = meta_dir;
        epub.oebps_dir = oebps_dir;
        epub.image_dir = image_dir;
        epub.text_dir = text_dir;

        epub.generate().await?;

        Ok(())
    }
}

impl DoclnCrawler {
    async fn set_epub_volumes(
        epub: &mut Epub,
        volume_tasks: &mut TaskManager<Result<(Volume, TaskManager<Result<Chapter>>)>>,
    ) -> Result<()> {
        let results = volume_tasks.wait().await;
        let (volumes, mut chapter_tasks): (Vec<_>, Vec<_>) = results
            .into_iter()
            .map(|r| {
                if let Ok((v, c)) = r {
                    (Ok(v), c)
                } else {
                    (r.map(|_| panic!("任务失败")), TaskManager::new())
                }
            })
            .unzip();
        let map_f = |r| {
            if let Ok(v) = r {
                v
            } else {
                panic!("任务失败")
            }
        };
        epub.volumes = volumes.into_iter().map(map_f).collect();

        for volume in epub.volumes.iter_mut().rev() {
            let chapters = chapter_tasks.pop().unwrap().wait().await;
            let map_f = |r| {
                if let Ok(c) = r {
                    c
                } else {
                    panic!("任务失败")
                }
            };
            volume.chapters = chapters.into_iter().map(map_f).collect();
            volume.chapters.sort_by_key(|c| c.index);
        }
        epub.volumes.sort_by_key(|v| v.index);
        Ok(())
    }

    fn volume_tasks(
        volumes: Vec<Volume>,
        processor: &Processor,
        downloader: &Downloader,
        parser: &Parser,
    ) -> TaskManager<Result<(Volume, TaskManager<Result<Chapter>>)>> {
        let mut task_manager: TaskManager<Result<(Volume, TaskManager<Result<Chapter>>)>> =
            TaskManager::new();
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
    ) -> TaskManager<Result<Chapter>> {
        let mut task_manager: TaskManager<Result<Chapter>> = TaskManager::new();
        for chapter in chapters {
            let downloader = downloader.clone();
            let parser = parser.clone();
            let processor = processor.clone();
            let chapter_future = Self::chapter_task(chapter, processor, downloader, parser);
            task_manager.spawn(chapter_future);
        }
        task_manager
    }

    fn volume_task(
        mut volume: Volume,
        processor: Processor,
        downloader: Downloader,
        parser: Parser,
    ) -> impl Future<Output = Result<(Volume, TaskManager<Result<Chapter>>)>> {
        async move {
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
            Ok((volume, chapter_tasks))
        }
    }

    fn chapter_task(
        mut chapter: Chapter,
        processor: Processor,
        downloader: Downloader,
        parser: Parser,
    ) -> impl Future<Output = Result<Chapter>> {
        async move {
            let chapter_html = downloader.chapter(&chapter.url).await?;
            let mut content = parser.chapter_content(chapter_html)?;
            if chapter.has_illustrations {
                let srcs = parser.chapter_srcs(&content);
                for src in srcs {
                    let Ok((image_bytes, extension)) = downloader.image(&src).await else {
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
            processor.write_chapter(content, &chapter).await?;
            Ok(chapter)
        }
    }
}
