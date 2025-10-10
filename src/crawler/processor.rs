use std::path::PathBuf;

use anyhow::{Ok, Result};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::epub::chapter::Chapter;

static XML_CONTENT_1: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>"#;

static XML_CONTENT_2: &str = r#"</title>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8"/>
</head>
<body>
    <h1>"#;

static XML_CONTENT_3: &str = r#"</h1>
    <div class="chapter-content">
"#;

static XML_CONTENT_4: &str = r#"    </div>
</body>
</html>"#;

#[derive(Clone)]
pub struct Processor {
    image_dir: PathBuf,
    text_dir: PathBuf,
}

impl Processor {
    pub fn new(image_dir: PathBuf, text_dir: PathBuf) -> Self {
        Self {
            image_dir,
            text_dir,
        }
    }

    pub async fn write_chapter(&self, chapter_content: String, chapter: &Chapter) -> Result<()> {
        // 创建XHTML内容 - 在body下创建div容器
        let mut xhtml_content = String::new();

        // XHTML头部
        xhtml_content.push_str(XML_CONTENT_1);
        xhtml_content.push_str(&chapter.title);
        xhtml_content.push_str(XML_CONTENT_2);
        xhtml_content.push_str(&chapter.title);
        xhtml_content.push_str(XML_CONTENT_3);
        // 添加章节内容
        xhtml_content.push_str(&chapter_content);
        // XHTML尾部
        xhtml_content.push_str(XML_CONTENT_4);

        let xhtml_path = self.text_dir.join(&chapter.filename);
        fs::write(&xhtml_path, xhtml_content).await?;

        println!("章节 XHTML 已保存到: {}", xhtml_path.display());

        Ok(())
    }

    pub async fn write_html(&self, html: String, chapter: &Chapter) -> Result<()> {
        let html_path = self.text_dir.join(&chapter.filename);
        fs::write(&html_path, html).await?;

        println!("章节 HTML 已保存到: {}", html_path.display());

        Ok(())
    }

    // 如果形参需要所有权，那么最好不要将形参声明为引用
    // 这样调用者就可以决定是克隆一个值还是直接传递所有权
    // 而不是声明一个引用然后在函数体内克隆
    // 用hash来命名插图文件，避免下载重复的插图
    // 不用hashmap缓存会导致重复计算hash，但是这样逻辑更简单
    pub async fn write_image(&self, image_bytes: Bytes, extension: String) -> Result<String> {
        println!("正在保存图片: {}", extension);
        let mut hasher = Sha256::new();
        hasher.update(&image_bytes);
        let hash = hasher.finalize();
        let filename = format!("{:x}.{}", hash, extension);
        let image_path = self.image_dir.join(&filename);
        if image_path.exists() {
            println!("重复图片: {}", image_path.display());
            return Ok(filename.to_string());
        }
        fs::write(&image_path, &image_bytes).await?;
        println!("图片已保存到: {}", image_path.display());
        Ok(filename.to_string())
    }
}
