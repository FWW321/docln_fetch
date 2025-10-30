pub mod chapter;
pub mod compression;
pub mod metadata;
pub mod volume;

pub use chapter::Chapter;
pub use compression::Compressor;
pub use metadata::Metadata;
use tracing::instrument;
pub use volume::Volume;

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum VolOrChap {
    Volumes(Vec<Volume>),
    Chapters(Vec<Chapter>),
}

impl Default for VolOrChap {
    fn default() -> Self {
        VolOrChap::Chapters(Vec::new())
    }
}

#[derive(Debug, Clone)]
pub struct Epub {
    pub id: String,
    pub title: String,
    pub lang: String,
    pub author: String,
    pub illustrator: Option<String>, // 插画师
    pub summary: String,             // 简介内容
    pub cover: Option<String>,       // 封面图片本地路径
    pub children: VolOrChap,         // 卷信息
    pub tags: Vec<String>,
    pub epub_dir: PathBuf,
    pub meta_dir: PathBuf,
    pub oebps_dir: PathBuf,
    pub image_dir: PathBuf,
    pub text_dir: PathBuf,
}

impl Epub {
    #[instrument(skip_all)]
    pub async fn generate(&self) -> Result<String> {
        tracing::info!("正在生成EPUB文件: {}", self.title);

        let metadata = Metadata::new();

        // 生成所有元数据文件
        metadata.generate(self).await?;

        // 压缩成EPUB文件
        let compressor = Compressor::new();
        let epub_filename = compressor.compress_epub(&self.epub_dir).await?;

        tracing::info!("EPUB文件生成成功: {}", epub_filename);
        Ok(epub_filename)
    }
}

impl Drop for Epub {
    fn drop(&mut self) {
        if self.epub_dir.exists() {
            // 删除EPUB文件夹
            tracing::info!("正在清理临时文件夹: {}", self.epub_dir.display());
            match std::fs::remove_dir_all(&self.epub_dir) {
                Ok(_) => tracing::info!("临时文件夹已删除: {}", self.epub_dir.display()),
                Err(e) => {
                    tracing::error!("删除临时文件夹时出错: {}: {}", self.epub_dir.display(), e)
                }
            }
        }
    }
}
