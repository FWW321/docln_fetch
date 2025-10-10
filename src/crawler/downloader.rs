use std::path::Path;
use std::sync::Arc;

use anyhow::{Ok, Result};
use bytes::Bytes;
use reqwest::{Client, StatusCode};
use tokio::fs;

#[derive(Clone)]
pub struct Downloader {
    client: Client,
    base_url: Arc<String>,
}

impl Downloader {
    pub fn new(base_url: String) -> Self {
        // #[derive(Clone)]
        // pub struct Client {
        //     inner: Arc<ClientRef>
        // }
        // #[derive(Clone)]会为每一个字段调用Clone
        // Client的Clone只是增加inner的引用计数，并不会克隆底层数据
        // 所以这里直接克隆Client是安全且高效的
        // 而new一个新的Client会重新建立连接池，浪费资源
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap();

        let base_url = Arc::new(base_url);

        // 定义重试策略：指数退避，最多重试3次
        // let retry_policy = ExponentialBackoff::builder().build_with_max_retries(MAX_RETRIES);
        // 创建重试中间件实例
        // let retry_middleware = RetryTransientMiddleware::new_with_policy(retry_policy);

        // #[derive(Clone, Default)]
        // pub struct ClientWithMiddleware {
        //     inner: reqwest::Client,                    // 核心 HTTP 客户端
        //     middleware_stack: Box<[Arc<dyn Middleware>]>,      // 中间件栈（请求处理）
        //     initialiser_stack: Box<[Arc<dyn RequestInitialiser>]>, // 初始化器栈（请求准备）
        // }
        //  middleware_stack 和 initialiser_stack 都是 Box 包裹的堆分配数组
        //  clone会分配堆内存并对每一个元素（即Arc）调用clone
        // ClientWithMiddleware的Clone开销较大，会涉及两次堆分配
        // 为了代码简洁，这里还是直接克隆ClientWithMiddleware
        // let client = ClientBuilder::new(client)
        //     .with(retry_middleware)
        //     .build();
        Self { client, base_url }
    }

    pub async fn novel_info(&self, novel_id: &str) -> Result<String> {
        let url = format!("{}/sang-tac/{}", self.base_url, novel_id);

        println!("正在获取: {}", url);

        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;

        Ok(html_content)
    }

    pub async fn image(&self, image_url: &str) -> Result<(Bytes, String)> {
        println!("下载图片: {}", image_url);
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        // 下载图片
        let response = self.client.get(image_url).send().await?;

        let image_bytes = response.bytes().await?;

        println!(
            "图片下载成功: {} ({} KB)",
            image_url,
            image_bytes.len() / 1024
        );

        Ok((image_bytes, extension.to_owned()))
    }

    pub async fn chapter(&self, chapter_url: &str) -> Result<String> {
        let chapter_url = if chapter_url.starts_with("/") {
            format!("{}{}", self.base_url, chapter_url)
        } else {
            chapter_url.to_owned()
        };

        // 请求过多（429）会被限制访问，需要控制访问频率或者使用代理
        println!("正在获取章节内容: {}", chapter_url);

        let response = self.client.get(chapter_url).send().await?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::TOO_MANY_REQUESTS => {
                let Some(retry_after) = response.headers().get("Retry-After") else {
                    return Err(anyhow::anyhow!("无法获取重试时间"));
                };
                println!("请等待 {} 秒后重试", retry_after.to_str().unwrap_or("未知"));
                return Err(anyhow::anyhow!("请求过多，已被限制访问"));
            }
            status => {
                return Err(anyhow::anyhow!("HTTP错误 {}", status));
            }
        }
        let html_content = response.text().await?;

        Ok(html_content)
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
