use anyhow::Result;
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};

use crate::crawler::TaskManager;

pub struct Compressor;

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor {
    pub fn new() -> Self {
        Self
    }

    /// 压缩EPUB文件夹为EPUB文件
    pub async fn compress_epub(&self, epub_dir: &Path) -> Result<String> {
        // 从目录名提取ID，目录名格式为 epub_{id}，转换为 docln_{id}
        let dir_name = epub_dir.file_name().unwrap().to_string_lossy();
        let filename = format!("{}.epub", dir_name);
        let epub_path = epub_dir.parent().unwrap().join(&filename);

        println!("正在压缩EPUB文件: {}", filename);

        // 创建ZIP文件
        let file = File::create(&epub_path).await?;
        let mut writer = ZipFileWriter::with_tokio(file);

        Self::add_mimetype(&mut writer, epub_dir).await?;
        Self::add_directory(&mut writer, epub_dir).await?;

        // 完成ZIP文件
        writer.close().await?;

        println!("EPUB文件已生成: {}", epub_path.display());

        // 删除EPUB文件夹
        println!("正在清理临时文件夹: {}", epub_dir.display());
        match fs::remove_dir_all(epub_dir).await {
            Ok(_) => println!("临时文件夹已删除: {}", epub_dir.display()),
            Err(e) => println!("删除临时文件夹时出错: {}: {}", epub_dir.display(), e),
        }

        Ok(filename)
    }

    async fn add_mimetype(writer: &mut ZipFileWriter<File>, dir: &Path) -> Result<()> {
        let path = dir.join("mimetype");
        let content = fs::read(&path).await?;

        // 验证mimetype内容
        // if content != b"application/epub+zip" {
        //     anyhow::bail!("Invalid mimetype content");
        // }

        let entry = ZipEntryBuilder::new("mimetype".into(), Compression::Stored);
        writer.write_entry_whole(entry, &content).await?;
        Ok(())
    }

    async fn add_directory(writer: &mut ZipFileWriter<File>, root_dir: &Path) -> Result<()> {
        // 创建任务管理器
        let mut task_manager = TaskManager::new();

        // 扫描目录并创建并发任务
        Self::scan_and_spawn_tasks(&mut task_manager, root_dir.to_path_buf()).await?;

        // 等待所有任务完成并收集结果
        let results = task_manager.wait().await?;

        // 将结果写入ZIP文件（按顺序保证稳定性）
        for (zip_path, content) in results {
            let entry = ZipEntryBuilder::new(zip_path.into(), Compression::Deflate);
            writer.write_entry_whole(entry, &content).await?;
        }

        Ok(())
    }

    async fn scan_and_spawn_tasks(
        task_manager: &mut TaskManager<(String, Vec<u8>)>,
        root_dir: PathBuf,
    ) -> Result<()> {
        // 使用栈存储待处理的目录和其在ZIP中的基础路径
        let mut stack = vec![(root_dir, String::new())];

        while let Some((current_dir, current_base_path)) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir).await?;

            // 先收集所有条目，稍后处理
            let mut sub_dirs = Vec::new();

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // 跳过已处理的特殊文件
                if name == "mimetype" && current_base_path.is_empty() {
                    continue;
                }

                // 构建ZIP中的路径
                let zip_path = if current_base_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", current_base_path, name)
                };

                if path.is_dir() {
                    // 记录子目录稍后处理
                    sub_dirs.push((path, zip_path));
                } else {
                    // 为每个文件创建并发任务
                    task_manager.spawn(async move {
                        let content = fs::read(&path).await?;
                        Ok::<_, anyhow::Error>((zip_path, content))
                    });
                }
            }

            // 将子目录逆序压入栈中（保证处理顺序）
            for (dir_path, zip_path) in sub_dirs.into_iter().rev() {
                stack.push((dir_path, zip_path));
            }
        }

        Ok(())
    }
}
