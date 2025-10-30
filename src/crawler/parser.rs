use anyhow::Result;
use scraper::element_ref::Select;
use scraper::{ElementRef, Html, Selector};
use tracing::{error, info, instrument};

use crate::config::{SiteConfig, get_site_config};
use crate::epub;
use crate::epub::chapter::Chapter;
use crate::extractor::{ChapterExtractor, Value, VolumeExtractor};
use crate::{Volume, epub::Epub};

#[derive(Clone, Copy)]
pub struct Parser {
    config: &'static SiteConfig,
}

impl Parser {
    pub fn new(site_name: &str) -> Self {
        Self {
            config: get_site_config(site_name).unwrap(),
        }
    }
}

impl Parser {
    #[instrument(skip_all)]
    pub fn chapter_content(&self, chapter: String) -> Result<String> {
        let document = Html::parse_document(&chapter);

        let content_extractor = &self
            .config
            .get_chapter_config()
            .ok_or_else(|| anyhow::anyhow!("未配置章节提取器"))?
            .content;

        let content_elem = document
            .select(&content_extractor.this)
            .next()
            .ok_or_else(|| anyhow::anyhow!("无法找到章节内容"))?;

        let content = content_extractor
            .extract_paragraphs(content_elem);

        if let Value::Single(content) = content {
            info!("章节内容提取完成");
            Ok(content)
        } else {
            error!("章节内容提取失败");
            Err(anyhow::anyhow!("章节内容提取失败"))
        }
    }

    pub fn chapter_srcs(&self, chapter_content: &str) -> Vec<String> {
        let mut srcs = Vec::new();
        let chapter_document = Html::parse_fragment(chapter_content);
        let img_selector = Selector::parse("img").expect("无法创建img选择器");

        for img_element in chapter_document.select(&img_selector) {
            let Some(src) = img_element.value().attr("src") else {
                continue;
            };
            if src.is_empty() {
                continue;
            }
            srcs.push(src.to_owned());
        }
        srcs
    }

    #[instrument(skip_all)]
    pub fn novel_info(&self, novel_html: &str, novel_id: String) -> Result<Epub> {
        info!("正在解析小说信息");
        let document = Html::parse_document(novel_html);

        let book_extractor = self.config.get_book_config();

        let Some(book_elem) = book_extractor.this(document.root_element()) else {
            anyhow::bail!("无法获取小说元素")
        };

        let Value::Single(title) = book_extractor.extract_title(book_elem) else {
            anyhow::bail!("无法提取小说标题");
        };

        let Value::Single(author) = book_extractor.extract_author(book_elem) else {
            anyhow::bail!("无法提取作者信息");
        };

        let tags = match book_extractor.extract_tags(book_elem) {
            Value::Multiple(ts) => ts,
            _ => Vec::new(),
        };

        let illustrator = match book_extractor.extract_illustrator(book_elem) {
            Value::Single(illust) => Some(illust),
            _ => None,
        };

        let cover = match book_extractor.extract_cover_url(book_elem) {
            Value::Single(cover_url) => Some(cover_url),
            _ => None,
        };

        let summary = match book_extractor.extract_summary(book_elem) {
            Value::Single(s) => s,
            _ => String::new(),
        };

        let children = self.children(book_elem)?;

        let epub = Epub {
            id: novel_id,
            title: title.trim().to_string(),
            lang: self.config.lang.clone(),
            author,
            illustrator,
            summary,
            cover,
            children,
            tags,
            epub_dir: Default::default(),
            meta_dir: Default::default(),
            oebps_dir: Default::default(),
            image_dir: Default::default(),
            text_dir: Default::default(),
        };

        info!("小说信息解析完成");
        Ok(epub)
    }

    pub fn children(&self, book_elem: ElementRef) -> Result<epub::VolOrChap> {
        let book_extractor = self.config.get_book_config();

        let mut result = Err(anyhow::anyhow!("未配置卷或章节提取器"));

        if let Some(volume_extractor) = &book_extractor.volumes {
            let volume_iter = book_elem.select(&volume_extractor.this);
            let volumes = self.volumes(volume_iter, volume_extractor)?;
            if volumes.is_empty() {
                if let Some(chapter_extractor) = &book_extractor.chapters {
                    let chapter_iter = book_elem.select(&chapter_extractor.this);
                    let chapters = self.chapters(chapter_iter, chapter_extractor, None)?;
                    result = Ok(epub::VolOrChap::Chapters(chapters))
                }
            } else {
                result = Ok(epub::VolOrChap::Volumes(volumes));
            }
        } else {
            if let Some(chapter_extractor) = &book_extractor.chapters {
                let chapter_iter = book_elem.select(&chapter_extractor.this);
                let chapters = self.chapters(chapter_iter, chapter_extractor, None)?;
                result = Ok(epub::VolOrChap::Chapters(chapters))
            }
        }

        result
    }

    #[instrument(skip_all)]
    pub fn volumes(&self, iter: Select, extractor: &VolumeExtractor) -> Result<Vec<Volume>> {
        info!("正在解析卷和章节信息");

        let mut volumes = Vec::new();

        for (volume_index, volume_elem) in iter.enumerate() {
            let Value::Single(title) = extractor.extract_title(volume_elem) else {
                anyhow::bail!("无法提取第 {} 卷标题", volume_index + 1);
            };

            let cover_url = match extractor.extract_cover_url(volume_elem) {
                Value::Single(url) => Some(url),
                _ => None,
            };

            let cover_chapter = Chapter {
                index: 0,
                title: title.trim().to_string(),
                url: String::new(),
                filename: format!("{}_cover.xhtml", volume_index + 1),
                images: Vec::new(),
            };

            let chapters = self.chapters(
                volume_elem.select(&extractor.chapters.this),
                &extractor.chapters,
                Some(volume_index),
            )?;

            volumes.push(Volume {
                index: volume_index + 1,
                cover: cover_url,
                chapters,
                cover_chapter,
            });
        }
        info!("卷和章节信息解析完成");
        Ok(volumes)
    }

    pub fn chapters(
        &self,
        iter: Select,
        extractor: &ChapterExtractor,
        volume_index: Option<usize>,
    ) -> Result<Vec<Chapter>> {
        let mut chapters = Vec::new();

        for (chapter_index, chapter_elem) in iter.enumerate() {
            let Value::Single(title) = extractor.extract_title(chapter_elem) else {
                if let Some(vol_idx) = volume_index {
                    anyhow::bail!(
                        "无法提取第 {} 卷第 {} 章标题",
                        vol_idx + 1,
                        chapter_index + 1
                    );
                } else {
                    anyhow::bail!("无法提取第 {} 章标题", chapter_index + 1);
                }
            };

            let Value::Single(url) = extractor.extract_content_url(chapter_elem) else {
                if let Some(vol_idx) = volume_index {
                    anyhow::bail!(
                        "无法提取第 {} 卷第 {} 章内容链接",
                        vol_idx + 1,
                        chapter_index + 1
                    );
                } else {
                    anyhow::bail!("无法提取第 {} 章内容链接", chapter_index + 1);
                }
            };

            let filename = if let Some(vol_idx) = volume_index {
                format!("{}_{}.xhtml", vol_idx + 1, chapter_index + 1)
            } else {
                format!("{}.xhtml", chapter_index + 1)
            };

            chapters.push(Chapter {
                index: chapter_index + 1,
                title: title.trim().to_string(),
                url,
                filename,
                images: Vec::new(),
            });
        }
        Ok(chapters)
    }
}
