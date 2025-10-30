use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use http::{Request, Response};
use reqwest::Body;
use reqwest::StatusCode;
use tower::{ServiceBuilder, ServiceExt as _};
use tower_http_client::{ResponseExt, ServiceExt as _};
use tower_reqwest::HttpClientLayer;
use tracing::{error, info, instrument};
use url::Url;

use crate::Chapter;
use crate::config::SiteConfig;
use crate::config::{AuthType, JAR, get_auth, get_site_config};
use crate::extractor::Value;

type HttpClient = tower::util::BoxCloneService<Request<Body>, Response<Body>, anyhow::Error>;

#[derive(Clone)]
pub struct Downloader {
    config: &'static SiteConfig,
    client: HttpClient,
    pub url: Arc<Url>,
}

impl Downloader {
    pub async fn chapters_sequential(&mut self, chapters: &[Chapter], next_url: &mut String) -> Result<Vec<String>> {
        let mut results = Vec::new();

        // let mut next_url = self.url.join(
        //     &chapters
        //         .first()
        //         .ok_or_else(|| anyhow::anyhow!("章节列表为空"))?
        //         .url,
        // )?;

        *next_url = self.url.join(next_url)?.to_string();

        let mut chapter_content = String::new();

        for chapter in chapters {
            let response = self.client.get(next_url.as_str()).send().await?;
            let chapter_html = response.body_reader().utf8().await?;

            let content_extract = &self
                .config
                .get_chapter_config()
                .expect("没有章节配置")
                .content;

            let chapter_html = scraper::Html::parse_document(&chapter_html);

            let content = chapter_html
                .select(&content_extract.this)
                .next()
                .ok_or_else(|| anyhow::anyhow!("无法找到章节内容"))?;

            let paragraphs = match content_extract.extract_paragraphs(content) {
                Value::Single(text) => text,
                _ => {
                    println!("content: {}", content.html());
                    return Err(anyhow::anyhow!("章节内容提取失败"))
                },
            };

            let title = match content_extract.extract_title(content) {
                Value::Single(text) => text.trim().to_string(),
                _ => chapter.title.clone(),
            };

            if content_extract.matches_title(&chapter.title, &title) {
                chapter_content.push_str(&paragraphs);
            } else {
                results.push(chapter_content);
                chapter_content = String::new();
                chapter_content.push_str(&paragraphs);
            }

            *next_url = match content_extract.extract_next_url(content) {
                Value::Single(url) => self.url.join(&url)?.to_string(),
                _ => {
                    tracing::error!("无法提取下一章节URL，结束下载");
                    return Ok(results);
                },
            };

            // 后续添加retry中间件
            let sleep_time = rand::random::<u64>() % 2000 + 1000;
            tokio::time::sleep(Duration::from_millis(sleep_time)).await;
        }

        Ok(results)
    }

    pub fn new(site_name: &str, url: String) -> Self {
        let config = get_site_config(site_name).expect("无法获取网站配置");

        let url = Url::parse(&url).expect("url解析错误");

        let url = Arc::new(url);

        let ua = ua_generator::ua::spoof_ua();

        let mut client_builder = reqwest::Client::builder()
            .user_agent(ua)
            .referer(true)
            .cookie_provider(JAR.clone());

        if let Some(auth_config) = get_auth().get(site_name) {
            match auth_config {
                AuthType::Token(token) => {
                    client_builder = client_builder.default_headers({
                        let mut headers = reqwest::header::HeaderMap::new();
                        headers.insert(
                            reqwest::header::AUTHORIZATION,
                            format!("Bearer {}", token)
                                .parse()
                                .expect("无法解析Authorization头"),
                        );
                        headers
                    });
                }
                _ => {}
            }
        }
        let client = client_builder.build().expect("无法构建HTTP客户端");

        let client = ServiceBuilder::new()
            .buffer(64)
            .rate_limit(
                config.rate_limit.num,
                Duration::from_secs(config.rate_limit.secs),
            )
            .concurrency_limit(config.concurrency_limit)
            .layer(HttpClientLayer) 
            .service(client)
            .map_err(|e| {
                error!("HTTP请求失败: {}", e);
                anyhow::anyhow!("HTTP请求失败: {}", e)
            })
            .boxed_clone();

        Self {
            client,
            url,
            config,
        }
    }

    #[instrument(skip_all)]
    pub async fn novel_info(&mut self) -> Result<String> {
        info!("正在获取: {}", self.url);

        let response = self.client.get(self.url.as_str()).send().await?;
        let html_content = response.body_reader().utf8().await?;

        Ok(html_content)
    }

    #[instrument(skip_all)]
    pub async fn image(&mut self, image_url: &str) -> Result<(Bytes, String)> {
        let image_url = self.url.join(image_url)?;
        info!("下载图片: {}", image_url);
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url.path())
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        let referer = if self.config.host.is_some() {
            let host = self.config.host.as_ref().unwrap();
            host
        } else {
            self.url.as_str()
        };

        // 下载图片
        let response = self.client
            .get(image_url.as_str())
            .header("Referer", referer)
            .send().await?;

        let image_bytes = response.body_reader().bytes().await?;

        info!("图片下载成功: {} KB", image_bytes.len() / 1024);

        Ok((image_bytes, extension.to_owned()))
    }

    #[instrument(skip_all)]
    pub async fn chapter(&mut self, chapter_url: &str) -> Result<String> {
        let chapter_url = self.url.join(chapter_url)?;

        // 请求过多（429）会被限制访问，需要控制访问频率或者使用代理
        info!("正在获取章节内容: {}", chapter_url);

        let response = self.client.get(chapter_url.as_str()).send().await?;
        match response.status() {
            StatusCode::OK => {
                info!("章节内容获取成功");
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let Some(retry_after) = response.headers().get("Retry-After") else {
                    return Err(anyhow::anyhow!("无法获取重试时间"));
                };
                error!(
                    "请求过多，已被限制访问，请等待 {} 秒后重试",
                    retry_after.to_str().unwrap_or("未知")
                );
                return Err(anyhow::anyhow!("请求过多，已被限制访问"));
            }
            status => {
                error!("HTTP错误 {}", status);
                return Err(anyhow::anyhow!("HTTP错误 {}", status));
            }
        }
        let html_content = response.body_reader().utf8().await?;

        Ok(html_content)
    }
}
