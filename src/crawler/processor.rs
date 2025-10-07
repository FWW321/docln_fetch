use std::path::PathBuf;

use anyhow::Result;
use reqwest::{Client, StatusCode};
use scraper::{Html, Selector};
use tokio::fs;

use super::CrawlerTaskManager;
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

pub struct ChapterProcessor {
    client: Client,
    base_url: String,
}

impl ChapterProcessor {
    pub fn new(client: Client, base_url: String) -> Self {
        Self { client, base_url }
    }

    pub fn fetch_chapter_content(
        &self,
        chapter_url: String,
        volume_index: usize,
        chapter_index: usize,
        chapter_title: String,
        images_dir: PathBuf,
        has_illustrations: bool,
    ) -> Result<String> {
        let xhtml_filename = format!("chapter_{:03}.xhtml", chapter_index + 1);

        let client = self.client.clone();

        // 返回相对路径（相对于OEBPS目录）
        let result = Ok(format!(
            "text/volume_{:03}/{}",
            volume_index + 1,
            xhtml_filename
        ));

        // 请求过多（429）会被限制访问，需要控制访问频率或者使用代理
        let future = async move {
            println!("正在获取章节内容: {}", chapter_url);

            let response = client
                .get(chapter_url)
                .send()
                .await
                .expect("无法获取章节内容");
            match response.status() {
                StatusCode::OK => {}
                StatusCode::TOO_MANY_REQUESTS => {
                    println!("章节请求过多，已被限制访问: HTTP 429");
                    let Some(retry_after) = response.headers().get("Retry-After") else {
                        println!("无法获取重试时间");
                        return;
                    };
                    println!("请等待 {} 秒后重试", retry_after.to_str().unwrap_or("未知"));
                    return;
                }
                status => {
                    println!(
                        "HTTP错误 {}: {}_{}",
                        status,
                        volume_index + 1,
                        chapter_index + 1
                    );
                    return;
                }
            }
            let html_content = response.text().await.expect("无法读取章节内容");

            let mut chapter_paragraphs = Vec::new();
            // Html结构体不是Send的，所以不能跨越await点
            // 需要在await点之前drop掉
            {
                let document = Html::parse_document(&html_content);

                // 提取章节内容
                let chapter_content_selector =
                    Selector::parse("div#chapter-content").expect("无法解析章节内容选择器");

                if let Some(content_div) = document.select(&chapter_content_selector).next() {
                    // 获取所有段落
                    let p_selector = Selector::parse("p").expect("无法解析段落选择器");
                    for p_element in content_div.select(&p_selector) {
                        chapter_paragraphs.push(p_element.html());
                    }
                }
            }

            // 根据章节是否有插图决定是否处理图片
            let modified_content: String = if has_illustrations {
                Self::download_chapter_illustrations(
                    client,
                    chapter_paragraphs,
                    images_dir.clone(),
                    chapter_index,
                    volume_index,
                )
                .await
                .expect("下载章节插图失败")
            } else {
                // 没有插图，直接使用原始段落内容
                chapter_paragraphs.join("\n")
            };

            // 创建XHTML内容 - 在body下创建div容器
            let mut xhtml_content = String::new();

            // XHTML头部
            xhtml_content.push_str(XML_CONTENT_1);
            xhtml_content.push_str(&chapter_title);
            xhtml_content.push_str(XML_CONTENT_2);
            xhtml_content.push_str(&chapter_title);
            xhtml_content.push_str(XML_CONTENT_3);
            // 添加章节内容
            xhtml_content.push_str(&modified_content);
            // XHTML尾部
            xhtml_content.push_str(XML_CONTENT_4);

            // 保存XHTML文件 - 按卷文件夹组织
            let mut volume_dir = images_dir.parent().expect("无法获取父目录").join("text");
            volume_dir.push(format!("volume_{:03}", volume_index + 1));
            fs::create_dir_all(&volume_dir)
                .await
                .expect("无法创建卷目录");

            let xhtml_path = volume_dir.join(&xhtml_filename);
            fs::write(&xhtml_path, xhtml_content)
                .await
                .expect("无法保存章节 XHTML 文件");

            println!("章节 XHTML 已保存到: {}", xhtml_path.display());
        };

        CrawlerTaskManager::spawn_chapter(future);

        result
    }

    pub fn fetch_and_process_chapters(
        &self,
        chapters: &mut Vec<Chapter>,
        volume_index: usize,
        images_dir: PathBuf,
    ) -> Result<()> {
        println!("\n正在处理第 {} 卷的章节内容...", volume_index + 1);

        for (chapter_index, chapter) in chapters.iter_mut().enumerate() {
            let full_chapter_url = if chapter.url.starts_with("/") {
                format!("{}{}", self.base_url, chapter.url)
            } else {
                chapter.url.clone()
            };

            match self.fetch_chapter_content(
                full_chapter_url,
                volume_index,
                chapter_index,
                chapter.title.clone(),
                images_dir.clone(),
                chapter.has_illustrations,
            ) {
                Ok(xhtml_path) => {
                    chapter.xhtml_path = Some(xhtml_path);
                    println!("  章节 '{}': 已处理", chapter.title);
                }
                Err(e) => {
                    println!("  章节 '{}' 处理失败: {}", chapter.title, e);
                    // 继续处理其他章节
                }
            }

            // 添加短暂延迟，避免请求过快
            // tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Ok(())
    }

    // 如果形参需要所有权，那么最好不要将形参声明为引用
    // 这样调用者就可以决定是克隆一个值还是直接传递所有权
    // 而不是声明一个引用然后在函数体内克隆
    // 用hash来命名插图文件，避免下载重复的插图
    // 图片不再有结构化的目录，直接存放在images目录下
    // 这样可以避免下载重复的图片，同时如果小说的目录结构不同（比如没有卷），也能使用相同逻辑替换本地图片
    async fn download_chapter_illustrations(
        client: Client,
        mut chapter_paragraphs: Vec<String>,
        images_dir: PathBuf,
        chapter_index: usize,
        volume_index: usize,
    ) -> Result<String> {
        let mut illustration_counter = 1;

        // 直接创建插图目录 - 按卷文件夹组织（因为进入这个函数的章节一定有插图）
        let volume_img_dir = images_dir.join(format!("volume_{:03}", volume_index + 1));
        let chapter_img_dir = volume_img_dir.join(format!("chapter_{:03}", chapter_index + 1));

        fs::create_dir_all(&chapter_img_dir).await?;

        // 处理每个段落
        for p_html in chapter_paragraphs.iter_mut() {
            // 解析段落HTML来查找图片
            let p_document = Html::parse_fragment(&p_html);
            let img_selector = Selector::parse("img").unwrap();

            for img_element in p_document.select(&img_selector) {
                let Some(img_src) = img_element.value().attr("src") else {
                    continue;
                };

                if img_src.is_empty() {
                    continue;
                }

                let result = Self::download_illustration(
                    client.clone(),
                    img_src,
                    chapter_img_dir.clone(),
                    illustration_counter,
                    volume_index,
                    chapter_index,
                );

                match result {
                    Ok(local_path) => {
                        // 替换原始src为本地路径（相对于images目录）
                        let original_img_html = img_element.html();
                        // 确保img标签正确闭合
                        let modified_img_html = if original_img_html.ends_with("/>") {
                            original_img_html.replace(img_src, &local_path)
                        } else {
                            original_img_html
                                .replace(img_src, &local_path)
                                .replace(">", "/>")
                        };
                        *p_html = p_html.replace(&original_img_html, &modified_img_html);

                        illustration_counter += 1;
                    }
                    Err(e) => {
                        println!("下载插图失败: {}", e);
                    }
                }
            }
        }

        Ok(chapter_paragraphs.join("\n"))
    }

    fn download_illustration(
        client: Client,
        image_url: &str,
        illustrations_dir: PathBuf,
        illustration_number: usize,
        volume_index: usize,
        chapter_index: usize,
    ) -> Result<String> {
        // 从URL中提取文件扩展名
        let extension = std::path::Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");

        // 插图命名为顺序编号
        let filename = format!("{:03}.{}", illustration_number, extension);
        let filepath = illustrations_dir.join(&filename);

        // 使用通用函数下载图片
        let image_url = image_url.to_owned();
        let future = async move {
            println!(
                "正在下载插图 {}_{}_{}: {}",
                volume_index + 1,
                chapter_index + 1,
                illustration_number,
                image_url
            );
            let response = client.get(image_url).send().await.expect("无法发送请求");
            let image_bytes = response.bytes().await.expect("无法读取响应");
            fs::write(&filepath, &image_bytes)
                .await
                .expect("无法保存插图文件");
            println!(
                "插图 {}_{}_{} 已保存到: {}",
                volume_index + 1,
                chapter_index + 1,
                illustration_number,
                filepath.display()
            );
        };

        CrawlerTaskManager::spawn_image(future);

        // 不需要等待所有插图下载完成，直接返回路径
        // 但是如果插图在主线程结束前没有下载完成，其他线程会被强制终止
        // 所以插图可能会下载不完整
        // 可以将任务的句柄收集到一个静态变量中，等主线程结束前等待这些任务完成
        // 返回正确的相对路径（从text/volume_XXX/chapter_XXX.xhtml到images/volume_XXX/chapter_XXX/）
        Ok(format!(
            "../../images/volume_{:03}/chapter_{:03}/{}",
            volume_index + 1,
            chapter_index + 1,
            filename
        ))
    }
}
