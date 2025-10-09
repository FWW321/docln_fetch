use std::collections::HashMap;

use crate::epub::chapter::Chapter;
use crate::{Volume, epub::Epub};
use anyhow::Result;
use scraper::{Element, Html, Selector};

pub struct Parser;

impl Parser {
    pub fn chapter_content(&self, chapter: String) -> Result<String> {
        let mut chapter_paragraphs = Vec::new();
        // Html结构体不是Send的，所以不能跨越await点
        // 需要在await点之前drop掉
        let document = Html::parse_document(&chapter);

        // 提取章节内容
        let chapter_content_selector = Selector::parse("div#chapter-content")
            .map_err(|e| anyhow::anyhow!("无法解析章节内容选择器: {}", e))?;

        if let Some(content_div) = document.select(&chapter_content_selector).next() {
            // 获取所有段落
            let p_selector =
                Selector::parse("p").map_err(|e| anyhow::anyhow!("无法解析段落选择器: {}", e))?;
            for p_element in content_div.select(&p_selector) {
                chapter_paragraphs.push(p_element.html());
            }
        }

        Ok(chapter_paragraphs.join("\n"))
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

    pub fn novel_title(&self, document: &Html) -> Result<String> {
        let title_selector = Selector::parse("span.series-name > a")
            .map_err(|e| anyhow::anyhow!("无法解析标题选择器: {}", e))?;

        let title = document
            .select(&title_selector)
            .next()
            .ok_or_else(|| anyhow::anyhow!("未找到小说标题"))?
            .text()
            .collect::<String>()
            .trim()
            .to_string();

        Ok(title)
    }

    pub fn summary(&self, document: &Html) -> Result<String> {
        let summary_selector = Selector::parse("div.summary-content > p")
            .map_err(|e| anyhow::anyhow!("无法解析简介选择器: {}", e))?;

        let summary_paragraphs: Vec<String> = document
            .select(&summary_selector)
            .map(|p| p.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
            .collect();

        if summary_paragraphs.is_empty() {
            return Err(anyhow::anyhow!("未找到简介内容"));
        }

        Ok(summary_paragraphs.join("\n"))
    }

    pub fn items(&self, document: &Html) -> Result<HashMap<String, String>> {
        let mut info_map = HashMap::new();
        let info_item_selector = Selector::parse("div.info-item")
            .map_err(|e| anyhow::anyhow!("无法解析信息项选择器: {}", e))?;
        let info_name_selector = Selector::parse("span.info-name")
            .map_err(|e| anyhow::anyhow!("无法解析信息名称选择器: {}", e))?;
        let info_value_selector = Selector::parse("span.info-value")
            .map_err(|e| anyhow::anyhow!("无法解析信息值选择器: {}", e))?;

        for info_item in document.select(&info_item_selector) {
            let Some(info_name_element) = info_item.select(&info_name_selector).next() else {
                continue;
            };
            let info_name = info_name_element
                .text()
                .collect::<String>()
                .trim()
                .to_string();

            let Some(info_value_element) = info_item.select(&info_value_selector).next() else {
                continue;
            };
            let info_value = info_value_element
                .text()
                .collect::<String>()
                .trim()
                .to_string();

            if !info_name.is_empty() && !info_value.is_empty() {
                info_map.insert(info_name, info_value);
            }
        }

        if info_map.is_empty() {
            return Err(anyhow::anyhow!("未找到任何信息项"));
        }

        Ok(info_map)
    }

    pub fn author(&self, document: &Html) -> Result<String> {
        let items = self.items(document)?;
        if let Some(author) = items.get("Tác giả:") {
            Ok(author.clone())
        } else {
            Err(anyhow::anyhow!("未找到作者信息"))
        }
    }

    pub fn illustrator(&self, document: &Html) -> Result<Option<String>> {
        let items = self.items(document)?;
        if let Some(illustrator) = items.get("Họa sĩ:") {
            if !illustrator.is_empty() {
                Ok(Some(illustrator.clone()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn tags(&self, document: &Html) -> Vec<String> {
        let mut tags = Vec::new();
        let tags_selector = Selector::parse("div.series-genres > a").expect("无法创建tags选择器");

        for tag_element in document.select(&tags_selector) {
            let tag_text = tag_element.text().collect::<String>().trim().to_string();
            if !tag_text.is_empty() {
                tags.push(tag_text);
            }
        }

        tags
    }

    pub fn novel_info(&self, novel_html: &str, novel_id: String) -> Result<Epub> {
        let document = Html::parse_document(novel_html);

        let title = self.novel_title(&document)?;

        let author = self.author(&document)?;

        let illustrator = self.illustrator(&document)?;

        let summary = self.summary(&document)?;

        let tags = self.tags(&document);

        let cover = self.cover_url(&document);

        let volumes = self.volumes(&document)?;

        // 创建Epub结构体（其他字段将在后续处理中填充）
        let epub = Epub {
            id: novel_id,
            title,
            author,
            illustrator,
            summary,
            cover,
            volumes,
            tags,
            epub_dir: Default::default(),
            meta_dir: Default::default(),
            oebps_dir: Default::default(),
            image_dir: Default::default(),
            text_dir: Default::default(),
        };

        Ok(epub)
    }

    pub fn cover_url(&self, document: &Html) -> Option<String> {
        let cover_selector = Selector::parse("div.content.img-in-ratio").unwrap();
        let cover_div = document.select(&cover_selector).next()?;
        let style = cover_div.value().attr("style")?;
        let start = style.find("url('")? + 5; // 跳过 "url('"
        let end = style[start..].find("')")?;

        let image_url = &style[start..start + end];

        if image_url.contains("nocover") {
            println!("使用默认封面图片，跳过下载");
            return None;
        }

        Some(image_url.to_string())
    }

    pub fn volumes(&self, document: &Html) -> Result<Vec<Volume>> {
        let mut volumes = Vec::new();
        let volume_infos = self.volume_info(document);
        for (index, (title, id)) in volume_infos.into_iter().enumerate() {
            let chapters = self.volume_chapters(document, &id, index);
            let cover_url = self.volume_cover_url(document, &id);
            let cover_chapter = Chapter {
                title,
                url: String::new(),
                has_illustrations: cover_url.is_some(),
                filename: format!("{}_cover.xhtml", index + 1),
                images: Vec::new(),
            };
            volumes.push(Volume {
                id,
                cover: cover_url,
                chapters,
                cover_chapter,
            });
        }
        Ok(volumes)
    }

    pub fn volume_info(&self, document: &Html) -> Vec<(String, String)> {
        let mut volumes = Vec::new();
        let list_vol_section_selector = Selector::parse("section#list-vol").unwrap();
        let list_volume_selector = Selector::parse("ol.list-volume").unwrap();
        let volume_item_selector = Selector::parse("li").unwrap();
        let volume_title_selector = Selector::parse("span.list_vol-title").unwrap();

        let Some(list_vol_section) = document.select(&list_vol_section_selector).next() else {
            return volumes;
        };

        let Some(list_volume) = list_vol_section.select(&list_volume_selector).next() else {
            return volumes;
        };

        for volume_item in list_volume.select(&volume_item_selector) {
            // 获取卷标题
            let volume_title = volume_item
                .select(&volume_title_selector)
                .next()
                .map(|span| span.text().collect::<String>().trim().to_string())
                .unwrap_or_else(|| "未知卷".to_string());

            // 获取卷的data-scrollto属性
            let volume_id = volume_item
                .value()
                .attr("data-scrollto")
                .unwrap_or("")
                .to_string();

            if !volume_id.is_empty() {
                volumes.push((volume_title, volume_id));
            }
        }

        volumes
    }

    pub fn volume_chapters(&self, document: &Html, volume_id: &str, index: usize) -> Vec<Chapter> {
        let mut chapters = Vec::new();

        // 根据volume_id找到对应的卷元素
        let volume_element_id = volume_id.trim_start_matches('#');
        let volume_header_selector =
            Selector::parse(&format!("header#{}", volume_element_id)).unwrap();
        let list_chapters_selector = Selector::parse("ul.list-chapters").unwrap();
        let chapter_item_selector = Selector::parse("li").unwrap();
        let chapter_name_selector = Selector::parse("div.chapter-name").unwrap();
        let chapter_link_selector = Selector::parse("a").unwrap();
        let illustration_icon_selector = Selector::parse("i").unwrap();

        let Some(volume_header) = document.select(&volume_header_selector).next() else {
            return chapters;
        };

        let Some(parent_element) = volume_header.parent_element() else {
            return chapters;
        };

        let Some(chapters_list) = parent_element.select(&list_chapters_selector).next() else {
            return chapters;
        };

        for (chapter_index, chapter_item) in
            chapters_list.select(&chapter_item_selector).enumerate()
        {
            // 查找章节名称和链接
            let Some(chapter_name_div) = chapter_item.select(&chapter_name_selector).next() else {
                continue;
            };

            let Some(chapter_link) = chapter_name_div.select(&chapter_link_selector).next() else {
                continue;
            };

            let chapter_title = chapter_link.text().collect::<String>().trim().to_string();

            let chapter_url = chapter_link.value().attr("href").unwrap_or("").to_string();

            // 检查是否包含插图图标
            let has_illustrations = chapter_name_div
                .select(&illustration_icon_selector)
                .next()
                .is_some();

            let filename = format!("{}_{}.xhtml", index + 1, chapter_index + 1);

            if !chapter_title.is_empty() && !chapter_url.is_empty() {
                chapters.push(Chapter {
                    title: chapter_title,
                    url: chapter_url,
                    has_illustrations,
                    filename,
                    images: Vec::new(),
                });
            }
        }

        chapters
    }

    pub fn volume_cover_url(&self, document: &Html, volume_id: &str) -> Option<String> {
        let volume_element_id = volume_id.trim_start_matches('#');
        let volume_header_selector =
            Selector::parse(&format!("header#{}", volume_element_id)).unwrap();
        let volume_cover_selector =
            Selector::parse("div.volume-cover div.content.img-in-ratio").unwrap();

        let volume_header = document.select(&volume_header_selector).next()?;
        let parent_element = volume_header.parent_element()?;
        let cover_div = parent_element.select(&volume_cover_selector).next()?;
        let style = cover_div.value().attr("style")?;
        let start = style.find("url('")? + 5; // 跳过 "url('"
        let end = style[start..].find("')")?;
        let image_url = &style[start..start + end];

        if image_url.contains("nocover") {
            println!("使用默认封面图片，跳过下载");
            return None;
        }

        Some(image_url.to_string())
    }
}
