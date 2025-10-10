pub mod chapter;
pub mod compression;
pub mod metadata;
pub mod volume;

pub use chapter::Chapter;
pub use compression::Compressor;
pub use metadata::Metadata;
pub use volume::Volume;

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Epub {
    pub id: String,
    pub title: String,
    pub author: String,
    pub illustrator: Option<String>, // 插画师
    pub summary: String,             // 简介内容
    pub cover: Option<String>,       // 封面图片本地路径
    pub volumes: Vec<Volume>,        // 卷信息
    pub tags: Vec<String>,
    pub epub_dir: PathBuf,
    pub meta_dir: PathBuf,
    pub oebps_dir: PathBuf,
    pub image_dir: PathBuf,
    pub text_dir: PathBuf,
}

impl Epub {
    pub async fn generate(&self) -> Result<String> {
        // 创建 EPUB 结构体

        let metadata = Metadata::new();

        // 生成所有元数据文件
        metadata.generate(self).await?;

        // 压缩成EPUB文件
        let compressor = Compressor::new();
        let epub_filename = compressor.compress_epub(&self.epub_dir).await?;

        println!("EPUB文件生成成功: {}", epub_filename);
        Ok(epub_filename)
    }
}
