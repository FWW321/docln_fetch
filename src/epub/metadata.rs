use anyhow::Result;
use tokio::fs;
use tracing::{info, instrument};

use super::Epub;

pub struct Metadata;

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl Metadata {
    pub fn new() -> Self {
        Self
    }

    /// 生成mimetype文件
    #[instrument(skip_all)]
    pub async fn mimetype(&self, epub: &Epub) -> Result<()> {
        info!("正在生成mimetype文件");
        let mimetype_content = "application/epub+zip";
        fs::write(epub.epub_dir.join("mimetype"), mimetype_content).await?;
        info!("mimetype文件生成完成");
        Ok(())
    }

    /// 生成container.xml文件
    #[instrument(skip_all)]
    pub async fn container_xml(&self, epub: &Epub) -> Result<()> {
        info!("正在生成container.xml文件");
        let container_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    </rootfiles>
</container>"#;
        fs::write(epub.meta_dir.join("container.xml"), container_content).await?;
        info!("container.xml文件生成完成");
        Ok(())
    }

    /// 生成content.opf文件
    #[instrument(skip_all)]
    pub async fn content_opf(&self, epub: &Epub) -> Result<()> {
        info!("正在生成content.opf文件");
        let mut content_opf = String::new();
        Self::opf_header(&mut content_opf);
        Self::opf_metadata(&mut content_opf, epub);
        Self::opf_manifest(&mut content_opf, epub);
        Self::opf_spine(&mut content_opf, epub);
        Self::opf_guide(&mut content_opf, epub);
        Self::opf_footer(&mut content_opf);

        fs::write(epub.oebps_dir.join("content.opf"), content_opf).await?;
        info!("content.opf文件生成完成");
        Ok(())
    }

    /// 生成toc.ncx文件
    #[instrument(skip_all)]
    pub async fn toc_ncx(&self, epub: &Epub) -> Result<()> {
        info!("正在生成toc.ncx文件");
        let mut toc_ncx = String::new();

        toc_ncx.push_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx version="2005-1" xmlns="http://www.daisy.org/z3986/2005/ncx/">
    <head>
        <meta name="dtb:uid" content=""#,
        );
        toc_ncx.push_str(&format!("docln:{}", epub.id));
        toc_ncx.push_str(
            r#""/>
        <meta name="dtb:depth" content="1"/>
        <meta name="dtb:totalPageCount" content="0"/>
        <meta name="dtb:maxPageNumber" content="0"/>
    </head>
    <docTitle>
        <text>"#,
        );
        toc_ncx.push_str(&epub.title);
        toc_ncx.push_str(
            r#"</text>
    </docTitle>
    <navMap>"#,
        );

        // 添加章节导航 - 层级结构
        let mut nav_point_counter = 1;
        for volume in &epub.volumes {
            if volume.chapters.is_empty() {
                continue;
            }

            // 卷作为一级导航点
            toc_ncx.push_str(&format!(
                r#"
        <navPoint id="navPoint{}" playOrder="{}">
            <navLabel>
                <text>{}</text>
            </navLabel>
            <content src="text/{}"/>"#,
                nav_point_counter,
                nav_point_counter,
                volume.cover_chapter.title,
                volume.cover_chapter.filename
            ));
            nav_point_counter += 1;

            // 章节作为卷的子导航点
            for chapter in &volume.chapters {
                toc_ncx.push_str(&format!(
                    r#"
            <navPoint id="navPoint{}" playOrder="{}">
                <navLabel>
                    <text>{}</text>
                </navLabel>
                <content src="text/{}"/>
            </navPoint>"#,
                    nav_point_counter, nav_point_counter, chapter.title, chapter.filename
                ));
                nav_point_counter += 1;
            }

            toc_ncx.push_str(
                r#"
        </navPoint>"#,
            );
        }

        toc_ncx.push_str(
            r#"
    </navMap>
</ncx>"#,
        );

        fs::write(epub.oebps_dir.join("toc.ncx"), toc_ncx).await?;
        info!("toc.ncx文件生成完成");
        Ok(())
    }

    /// 生成所有元数据文件
    #[instrument(skip_all)]
    pub async fn generate(&self, epub: &Epub) -> Result<()> {
        info!("正在生成EPUB元数据文件");
        // 生成所有元数据文件
        self.mimetype(epub).await?;
        self.container_xml(epub).await?;
        self.content_opf(epub).await?;
        self.toc_ncx(epub).await?;

        info!("EPUB元数据文件已生成");
        Ok(())
    }
}

impl Metadata {
    fn opf_header(content_opf: &mut String) {
        content_opf.push_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">"#,
        );
    }

    #[instrument(skip_all)]
    fn opf_metadata(content_opf: &mut String, epub: &Epub) {
        info!("正在生成opf的metadata部分");
        content_opf.push_str(
            r#"
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
        <dc:identifier id="BookId">docln:"#,
        );
        content_opf.push_str(&epub.id.to_string());
        content_opf.push_str(
            r#"</dc:identifier>
        <dc:title>"#,
        );
        content_opf.push_str(&epub.title);
        content_opf.push_str(
            r#"</dc:title>
        <dc:language>vi</dc:language>
        <dc:creator opf:role="aut">"#,
        );
        content_opf.push_str(&epub.author);
        content_opf.push_str(r#"</dc:creator>"#);

        // 添加插画师信息
        if let Some(illustrator) = &epub.illustrator {
            content_opf.push_str(
                r#"
        <dc:contributor opf:role="ill">"#,
            );
            content_opf.push_str(illustrator);
            content_opf.push_str(r#"</dc:contributor>"#);
        }

        // 添加标签
        for tag in &epub.tags {
            content_opf.push_str(
                r#"
        <dc:subject>"#,
            );
            content_opf.push_str(tag);
            content_opf.push_str(r#"</dc:subject>"#);
        }

        // 添加简介
        if !epub.summary.is_empty() {
            content_opf.push_str(
                r#"
        <dc:description>"#,
            );
            content_opf.push_str(&epub.summary);
            content_opf.push_str(r#"</dc:description>"#);
        }

        content_opf.push_str(
            r#"
        <dc:publisher>docln-fetch</dc:publisher>
        <dc:date>"#,
        );
        content_opf.push_str(&chrono::Local::now().format("%Y-%m-%d").to_string());
        content_opf.push_str(
            r#"</dc:date>
        <meta name="generator" content="docln-fetch"/>
    </metadata>"#,
        );
        info!("opf的metadata部分生成完成");
    }

    #[instrument(skip_all)]
    fn opf_manifest(content_opf: &mut String, epub: &Epub) {
        info!("正在生成opf的manifest部分");
        // manifest内容
        content_opf.push_str(
            r#"
            <manifest>
        <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#,
        );

        if let Some(cover_name) = &epub.cover {
            content_opf.push_str(&format!(
                r#"
        <item id="cover-image" href="images/{}" media-type="{}"/>"#,
                cover_name,
                Self::get_media_type(cover_name)
            ));
        }

        // 添加章节文件
        for (i, volume) in epub.volumes.iter().enumerate() {
            // 添加卷封面图片
            if let Some(cover_name) = &volume.cover {
                content_opf.push_str(&format!(
                    r#"
        <item id="vol{}-cover-img" href="images/{}" media-type="{}"/>"#,
                    i + 1,
                    cover_name,
                    Self::get_media_type(cover_name)
                ));
            }
            // 为有卷封面的卷添加章节0
            if volume.cover.is_some() {
                content_opf.push_str(&format!(
                    r#"
        <item id="vol{}-cover" href="text/{}" media-type="application/xhtml+xml"/>"#,
                    i + 1,
                    volume.cover_chapter.filename
                ));
            }

            for (j, chapter) in volume.chapters.iter().enumerate() {
                for image_name in &chapter.images {
                    content_opf.push_str(&format!(
                        r#"
        <item id="img-{}" href="images/{}" media-type="{}"/>"#,
                        image_name,
                        image_name,
                        Self::get_media_type(image_name)
                    ));
                }
                content_opf.push_str(&format!(
                    r#"
        <item id="chap{}-{}" href="text/{}" media-type="application/xhtml+xml"/>"#,
                    i + 1,
                    j + 1,
                    chapter.filename
                ));
            }
        }
        content_opf.push_str(r#"    </manifest>"#);
        info!("opf的manifest部分生成完成");
    }

    #[instrument(skip_all)]
    fn opf_spine(content_opf: &mut String, epub: &Epub) {
        info!("正在生成opf的spine部分");
        // spine内容
        content_opf.push_str(
            r#"
    <spine toc="ncx">"#,
        );

        // 添加章节到spine - 按卷的顺序添加
        for (i, volume) in epub.volumes.iter().enumerate() {
            // 没有封面的卷跳过
            if volume.cover.is_some() {
                content_opf.push_str(&format!(
                    r#"
        <itemref idref="vol{}-cover"/>"#,
                    i + 1
                ));
            }

            for (j, _) in volume.chapters.iter().enumerate() {
                content_opf.push_str(&format!(
                    r#"
        <itemref idref="chap{}-{}"/>"#,
                    i + 1,
                    j + 1
                ));
            }
        }

        content_opf.push_str(
            r#"
    </spine>"#,
        );
        info!("opf的spine部分生成完成");
    }

    #[instrument(skip_all)]
    fn opf_guide(content_opf: &mut String, epub: &Epub) {
        info!("正在生成opf的guide部分");
        let Some(cover_name) = &epub.cover else {
            return;
        };
        content_opf.push_str(&format!(
            r#"
    <guide>
        <reference type="cover" title="Cover" href="images/{}"/>
    </guide>"#,
            cover_name
        ));
        info!("opf的guide部分生成完成");
    }

    fn opf_footer(content_opf: &mut String) {
        content_opf.push_str(r#"</package>"#);
    }

    fn get_media_type(filename: &str) -> &str {
        if filename.ends_with(".png") {
            "image/png"
        } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
            "image/jpeg"
        } else {
            "application/octet-stream"
        }
    }
}
