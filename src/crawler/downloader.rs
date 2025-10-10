use std::path::Path;
use std::sync::Arc;

use anyhow::{Ok, Result};
use bytes::Bytes;
use reqwest::{Client, StatusCode};
use tracing::{error, info, instrument};

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

    #[instrument(skip_all)]
    pub async fn novel_info(&self, novel_id: &str) -> Result<String> {
        let url = format!("{}/sang-tac/{}", self.base_url, novel_id);

        info!("正在获取: {}", url);

        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;

        Ok(html_content)
    }

    #[instrument(skip_all)]
    pub async fn image(&self, image_url: &str) -> Result<(Bytes, String)> {
        info!("下载图片: {}", image_url);
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        // 下载图片
        let response = self.client.get(image_url).send().await?;

        let image_bytes = response.bytes().await?;

        info!(
            "图片下载成功: {} KB",
            image_bytes.len() / 1024
        );

        Ok((image_bytes, extension.to_owned()))
    }

    #[instrument(skip_all)]
    pub async fn chapter(&self, chapter_url: &str) -> Result<String> {
        let chapter_url = if chapter_url.starts_with("/") {
            format!("{}{}", self.base_url, chapter_url)
        } else {
            chapter_url.to_owned()
        };

        // 请求过多（429）会被限制访问，需要控制访问频率或者使用代理
        info!("正在获取章节内容: {}", chapter_url);

        let response = self.client.get(chapter_url).send().await?;
        match response.status() {
            StatusCode::OK => {
                info!("章节内容获取成功");
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let Some(retry_after) = response.headers().get("Retry-After") else {
                    return Err(anyhow::anyhow!("无法获取重试时间"));
                };
                error!("请求过多，已被限制访问，请等待 {} 秒后重试", retry_after.to_str().unwrap_or("未知"));
                return Err(anyhow::anyhow!("请求过多，已被限制访问"));
            }
            status => {
                error!("HTTP错误 {}", status);
                return Err(anyhow::anyhow!("HTTP错误 {}", status));
            }
        }
        let html_content = response.text().await?;

        Ok(html_content)
    }
}
