use anyhow::Result;

use docln_fetch::config::get_site_config;
use docln_fetch::{DoclnCrawler, get_user_input, logger};

#[tokio::main]
async fn main() -> Result<()> {
    logger::init();

    loop {
        println!("\n=== docln-fetch ===");
        let site = get_user_input("请输入要爬取的网站")?;

        let (id, url) = get_site_config(&site)?.build_url();

        let crawler = DoclnCrawler::new(url, &site);

        let Some(id) = id else {
            println!("没有找到小说id, 请重试");
            continue;
        };

        crawler.crawl(id, site).await?;

        let continue_choice = get_user_input("是否继续爬取其他小说? (y/n): ")?;

        if continue_choice.trim().to_lowercase() != "y" {
            break;
        }
    }

    Ok(())
}
