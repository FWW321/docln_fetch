use std::io;

use anyhow::Result;
use tracing::{debug, info, instrument};

#[instrument]
pub fn get_user_input() -> Result<Vec<String>> {
    println!("请输入小说ID(以空格分割): ");
    let mut ids = String::new();
    io::stdin().read_line(&mut ids)?;
    debug!("用户输入: {}", ids);
    let ids = ids.split_whitespace().map(|s| s.to_owned()).collect();
    Ok(ids)
}

#[instrument]
pub fn display_elapsed_time(duration: std::time::Duration) {
    let total_ms = duration.as_millis();

    if total_ms >= 60000 {
        // 超过1分钟：显示分秒
        let mins = total_ms / 60000;
        let secs = (total_ms % 60000) / 1000;
        let ms_remaining = total_ms % 1000;

        if ms_remaining > 0 {
            info!(
                "✅ 爬取完成！耗时: {}分{}秒{}毫秒",
                mins, secs, ms_remaining
            );
        } else {
            info!("✅ 爬取完成！耗时: {}分{}秒", mins, secs);
        }
    } else if total_ms >= 1000 {
        // 1秒到1分钟：显示秒和毫秒
        let secs = total_ms / 1000;
        let ms_remaining = total_ms % 1000;

        if ms_remaining > 0 {
            info!("✅ 爬取完成！耗时: {}秒{}毫秒", secs, ms_remaining);
        } else {
            info!("✅ 爬取完成！耗时: {}秒", secs);
        }
    } else {
        // 少于1秒：只显示毫秒
        info!("✅ 爬取完成！耗时: {}毫秒", total_ms);
    }
}
