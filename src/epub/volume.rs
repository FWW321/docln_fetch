use crate::epub::chapter::Chapter;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub struct VolumeBuilder {
    title: String,
    volume_id: String,
    cover_image_path: Option<String>,
    chapters: Vec<Chapter>,
}

impl VolumeBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            volume_id: String::new(),
            cover_image_path: None,
            chapters: Vec::new(),
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn volume_id(mut self, volume_id: String) -> Self {
        self.volume_id = volume_id;
        self
    }

    pub fn cover_image_path(mut self, path: Option<String>) -> Self {
        self.cover_image_path = path;
        self
    }

    pub fn chapters(mut self, chapters: Vec<Chapter>) -> Self {
        self.chapters = chapters;
        self
    }

    pub fn build(self) -> Volume {
        Volume {
            title: self.title,
            volume_id: self.volume_id,
            cover_image_path: self.cover_image_path,
            chapters: self.chapters,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub title: String,
    pub volume_id: String,
    pub cover_image_path: Option<String>,
    pub chapters: Vec<Chapter>,
}

impl Volume {
    pub fn builder() -> VolumeBuilder {
        VolumeBuilder::new()
    }

    pub fn generate_volume_cover_chapter(
        &self,
        volume_index: usize,
        oebps_dir: &Path,
    ) -> Result<()> {
        let volume_dir = oebps_dir
            .join("text")
            .join(format!("volume_{:03}", volume_index + 1));
        fs::create_dir_all(&volume_dir)?;

        let chapter_filename = format!("chapter_000.xhtml");
        let chapter_path = volume_dir.join(chapter_filename);

        let mut xhtml_content = String::new();
        xhtml_content.push_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>"#,
        );

        xhtml_content.push_str(&self.title);
        xhtml_content.push_str(
            r#"</title>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8"/>
</head>
<body>
    <div class="cover">
        <h1>"#,
        );

        xhtml_content.push_str(&self.title);
        xhtml_content.push_str(
            r#"</h1>
"#,
        );

        // 插入封面图片
        if let Some(ref cover_path) = self.cover_image_path {
            // 计算相对路径（假设cover_path已是相对OEBPS的路径）
            xhtml_content.push_str(&format!(
                "        <img src=\"../../{}\" alt=\"封面\" class=\"volume-cover-img\"/>",
                cover_path
            ));
            xhtml_content.push_str("\n");
        }

        xhtml_content.push_str(
            r#"    </div>
</body>
</html>"#,
        );

        fs::write(&chapter_path, xhtml_content)?;
        println!(
            "卷 '{}' 封面章节已生成: {}",
            self.title,
            chapter_path.display()
        );
        Ok(())
    }
}
