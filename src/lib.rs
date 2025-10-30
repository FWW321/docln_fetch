pub mod config;
pub mod crawler;
pub mod epub;
pub mod extractor;
pub mod logger;
pub mod utils;

pub use crawler::DoclnCrawler;
pub use epub::{Chapter, Epub, Volume};
pub use utils::get_user_input;
