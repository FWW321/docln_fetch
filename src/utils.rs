use std::io;

use anyhow::Result;
use tracing::{debug, instrument};

#[instrument]
pub fn get_user_input(prompt: &str) -> Result<String> {
    println!("{}: ", prompt);
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    debug!("用户输入: {}", input);
    Ok(input.trim().to_owned())
}
