use std::io::{self, Write};
use std::time::Instant;

use anyhow::Result;
use tracing::Level;

use docln_fetch::{DoclnCrawler, get_user_input, display_elapsed_time};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let crawler = DoclnCrawler::new();

    loop {
        println!("\n=== docln-fetch ===");
        match get_user_input() {
            Ok(ids) => {
                let start = Instant::now();
                crawler.crawl(ids).await?;

                let duration = start.elapsed();
                display_elapsed_time(duration);
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
    Ok(())
}
