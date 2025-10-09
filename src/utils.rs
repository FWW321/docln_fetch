use anyhow::Result;
use std::io;

pub fn get_user_input() -> Result<String> {
    println!("请输入小说ID: ");
    let mut novel_id = String::new();
    io::stdin().read_line(&mut novel_id)?;

    Ok(novel_id.trim().to_owned())
}
