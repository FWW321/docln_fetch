use std::path::Path;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tokio::fs;
use anyhow::Result;
use reqwest::Client;
use sha2::{Digest, Sha256};

// 内容哈希 -> 文件名 的映射
static CONTENT_HASH_MAP: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub async fn download_image(
    client: &Client,
    image_url: &str,
    illustrations_dir: &Path,
) -> Result<String> {
    // 初始化哈希映射
    let hash_map = CONTENT_HASH_MAP.get_or_init(|| Mutex::new(HashMap::new()));

    println!("下载图片: {}", image_url);

    // 下载图片
    let response = client.get(image_url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("下载失败 {}: {}", image_url, e))?;
    
    let image_bytes = response.bytes()
        .await
        .map_err(|e| anyhow::anyhow!("读取响应失败 {}: {}", image_url, e))?;

    // 计算内容哈希
    let mut hasher = Sha256::new();
    hasher.update(&image_bytes);
    let content_hash = format!("{:x}", hasher.finalize());

    let mut hash_map_lock = hash_map.lock().unwrap();

    // 检查是否已有相同内容的图片
    if let Some(existing_filename) = hash_map_lock.get(&content_hash) {
        println!("发现重复图片内容: {} -> 复用 {}", image_url, existing_filename);
        return Ok(existing_filename.clone());
    }

    // 新图片，生成文件名并保存
    let extension = std::path::Path::new(image_url)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    
    // 使用内容哈希作为文件名，确保唯一性
    let filename = format!("{}.{}", &content_hash[..16], extension);
    let filepath = illustrations_dir.join(&filename);

    // 保存图片
    fs::write(&filepath, &image_bytes).await
        .map_err(|e| anyhow::anyhow!("保存图片失败 {}: {}", filepath.display(), e))?;

    // 注册到哈希映射
    hash_map_lock.insert(content_hash, filename.clone());

    println!("图片保存成功: {}", filename);
    Ok(filename)
}

pub struct ImageDownloader {
    client: Client,
}

impl ImageDownloader {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// 通用的图片下载函数
    pub async fn download_image(
        &self,
        image_url: &str,
        filepath: &Path,
        log_prefix: &str,
    ) -> Result<()> {
        println!("正在下载{}图片: {}", log_prefix, image_url);

        // 下载图片
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;

        // 保存到本地
        fs::write(filepath, &image_bytes).await?;

        println!("{}图片已保存到: {}", log_prefix, filepath.display());
        Ok(())
    }

    /// 通用的封面图片下载函数
    pub async fn download_cover_image_common(
        &self,
        image_url: &str,
        images_dir: &Path,
        filename: &str,
        log_prefix: &str,
        skip_default: bool,
    ) -> Result<Option<String>> {
        // 检查是否为默认的nocover图片
        if skip_default && image_url.contains("nocover") {
            println!("{}使用默认封面图片，跳过下载", log_prefix);
            return Ok(None);
        }

        let filepath = images_dir.join(filename);

        // 使用通用函数下载图片
        self.download_image(image_url, &filepath, log_prefix)
            .await?;

        println!(
            "{}封面图片已保存到: {} (文件名: {})",
            log_prefix,
            filepath.display(),
            filename
        );

        // 返回相对路径（相对于OEBPS目录）
        Ok(Some(format!("images/{}", filename)))
    }

    pub async fn download_novel_cover(
        &self,
        image_url: &str,
        _novel_id: u32,
        _title: &str,
        epub_dir: &Path,
    ) -> Result<Option<String>> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        // EPUB标准目录结构: OEBPS/images/
        let images_dir = epub_dir.join("OEBPS").join("images");
        fs::create_dir_all(&images_dir).await?;

        // 小说封面命名为cover
        let filename = format!("cover.{}", extension);

        // 使用通用函数下载封面图片
        self.download_cover_image_common(image_url, &images_dir, &filename, "小说", true)
            .await
    }

    pub async fn download_volume_cover_image(
        &self,
        image_url: &str,
        _volume_index: usize,
        volume_title: &str,
        epub_dir: &Path,
    ) -> Result<Option<String>> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        // 清理卷标题中的特殊字符，用于文件名，并添加编号
        let safe_volume_title = volume_title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .replace(' ', "_");

        // EPUB标准目录结构: OEBPS/images/
        let images_dir = epub_dir.join("OEBPS").join("images");
        fs::create_dir_all(&images_dir).await?;

        // 卷封面命名为卷名
        let filename = format!("{}.{}", safe_volume_title, extension);

        // 使用通用函数下载卷封面图片
        self.download_cover_image_common(
            image_url,
            &images_dir,
            &filename,
            &format!("卷 '{}' ", volume_title),
            true,
        )
        .await
    }
}
