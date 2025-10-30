use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, LazyLock},
    u64,
};

use anyhow::Result;
use reqwest::cookie::Jar;
use serde::Deserialize;
use url::Url;

use crate::extractor::{BookExtractor, ChapterExtractor};

static SITE_CONFIG_DIR: &str = "config";

static CONFIG: LazyLock<Config> = LazyLock::new(|| init_auth_config().expect("配置初始化失败"));

pub static JAR: LazyLock<Arc<Jar>> = LazyLock::new(|| CONFIG.get_jar());

static SITE_CONFIG: LazyLock<HashMap<String, SiteConfig>> = LazyLock::new(|| {
    init_site_config().unwrap_or_else(|e| {
        panic!("网站配置初始化失败: {}", e);
    })
});

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Config {
    #[serde(default)]
    pub auth: HashMap<String, AuthType>,
}

impl Config {
    // todo: 统一使用reqwest的Url
    pub fn get_jar(&self) -> Arc<Jar> {
        let jar = Jar::default();
        for name in SITE_CONFIG.keys() {
            if let Some(auth_type) = self.auth.get(name) {
                if let AuthType::Cookies(cookies) = auth_type {
                    for (key, value) in cookies {
                        let url = Url::parse(SITE_CONFIG[name].base_url.as_str()).unwrap();
                        let url = format!("{}://{}", url.scheme(), url.host_str().unwrap());
                        let url = reqwest::Url::parse(&url).unwrap();
                        jar.add_cookie_str(&format!("{}={}", key, value), &url);
                    }
                    return Arc::new(jar);
                }
            }
        }
        Arc::new(jar)
    }
}

#[derive(Deserialize)]
pub enum AuthType {
    // Basic { username: String, password: String },
    Token(String),
    Cookies(HashMap<String, String>),
}

pub fn init_auth_config() -> Result<Config> {
    config::Config::builder()
        .add_source(config::File::with_name("config").format(config::FileFormat::Toml))
        .build()?
        .try_deserialize()
        .map_err(|e| anyhow::anyhow!("配置文件反序列化失败: {}", e))
}

pub fn get_auth() -> &'static HashMap<String, AuthType> {
    &CONFIG.auth
}

pub fn get_site_config(name: &str) -> Result<&'static SiteConfig> {
    SITE_CONFIG
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("配置 '{}' 不存在", name))
}

fn init_site_config() -> Result<HashMap<String, SiteConfig>> {
    let site_config_dir = std::path::Path::new(SITE_CONFIG_DIR);
    if !(site_config_dir.exists() && site_config_dir.is_dir()) {
        anyhow::bail!("配置目录 {} 不存在", SITE_CONFIG_DIR);
    }

    let mut configs = HashMap::new();

    for entry in std::fs::read_dir(site_config_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
            let config = SiteConfig::load(&path)?;
            configs.insert(config.name.clone(), config);
        }
    }
    Ok(configs)
}

#[derive(Deserialize)]
pub struct SiteConfig {
    pub name: String,
    pub rate_limit: RateLimit,
    pub host: Option<String>,
    #[serde(default = "default_concurrency_limit")]
    pub concurrency_limit: usize,
    pub base_url: String,
    pub lang: String,
    pub book: BookExtractor,
}

#[derive(Deserialize, Clone, Copy)]
pub struct RateLimit {
    pub num: u64,
    pub secs: u64,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            num: u64::MAX,
            secs: 1,
        }
    }
}

fn default_concurrency_limit() -> usize {
    usize::MAX
}

impl SiteConfig {
    pub fn load(config_path: &Path) -> Result<Self> {
        let file_content = std::fs::read_to_string(config_path)?;

        config::Config::builder()
            .add_source(config::File::from_str(
                &file_content,
                config::FileFormat::Toml,
            ))
            .build()?
            .try_deserialize()
            .map_err(|e| anyhow::anyhow!("{}文件反序列化失败: {}", config_path.display(), e))
    }

    pub fn build_url(&self) -> (Option<String>, String) {
        let params = self.extract_params();
        if params.is_empty() {
            return (None, self.base_url.to_string());
        }

        let mut values = HashMap::new();
        for param in params {
            println!("请输入 {} :", param);
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            values.insert(param, input.trim().to_string());
        }
        (values.get("id").cloned(), self.replace_params(values))
    }

    fn extract_params(&self) -> Vec<String> {
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        let mut params = HashSet::new();

        for cap in re.captures_iter(&self.base_url.as_str()) {
            params.insert(cap[1].to_string());
        }

        params.into_iter().collect()
    }

    fn replace_params(&self, values: HashMap<String, String>) -> String {
        let re = regex::Regex::new(r"\{(\w+)\}").unwrap();
        re.replace_all(&self.base_url.as_str(), |caps: &regex::Captures| {
            values
                .get(&caps[1])
                .unwrap_or(&caps[0].to_string())
                .to_string()
        })
        .to_string()
    }

    pub fn get_book_config(&self) -> &BookExtractor {
        &self.book
    }

    pub fn get_chapter_config(&self) -> Option<&ChapterExtractor> {
        let mut result = None;
        if let Some(volume_extractor) = &self.book.volumes {
            result = Some(&volume_extractor.chapters);
        } else {
            if let Some(chapter_extractor) = &self.book.chapters {
                result = Some(chapter_extractor);
            }
        };
        result
    }
}
