use std::io::{self, Write};
use std::time::Instant;

use anyhow::Result;

use docln_fetch::{DoclnCrawler, get_user_input};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let crawler = DoclnCrawler::new();

    loop {
        println!("\n=== docln-fetch ===");
        match get_user_input() {
            Ok(novel_id) => {
                println!("\n正在爬取 ID为 {} 的小说...", novel_id);
                let start = Instant::now();
                crawler.generate_epub(novel_id).await?;

                let duration = start.elapsed();
                let total_ms = duration.as_millis();

                if total_ms >= 60000 {
                    // 超过1分钟：显示分秒
                    let mins = total_ms / 60000;
                    let secs = (total_ms % 60000) / 1000;
                    let ms_remaining = total_ms % 1000;

                    if ms_remaining > 0 {
                        println!(
                            "✅ 爬取完成！耗时: {}分{}秒{}毫秒",
                            mins, secs, ms_remaining
                        );
                    } else {
                        println!("✅ 爬取完成！耗时: {}分{}秒", mins, secs);
                    }
                } else if total_ms >= 1000 {
                    // 1秒到1分钟：显示秒和毫秒
                    let secs = total_ms / 1000;
                    let ms_remaining = total_ms % 1000;

                    if ms_remaining > 0 {
                        println!("✅ 爬取完成！耗时: {}秒{}毫秒", secs, ms_remaining);
                    } else {
                        println!("✅ 爬取完成！耗时: {}秒", secs);
                    }
                } else {
                    // 少于1秒：只显示毫秒
                    println!("✅ 爬取完成！耗时: {}毫秒", total_ms);
                }
            }
            Err(e) => {
                println!("输入错误: {}", e);
            }
        }

        print!("\n是否继续爬取其他小说? (y/n): ");
        io::stdout().flush()?;
        let mut continue_choice = String::new();
        io::stdin().read_line(&mut continue_choice)?;
        if continue_choice.trim().to_lowercase() != "y" {
            break;
        }
    }

    println!("程序结束。");
    Ok(())
}
