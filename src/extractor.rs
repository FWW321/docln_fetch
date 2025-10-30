pub mod attr;
pub mod combine;
pub mod html;
pub mod list;
pub mod next;
pub mod text;
pub mod url;
pub mod current;

use regex::Regex;
use scraper::{ElementRef, Selector, element_ref::Select};
use serde::{Deserialize, Deserializer};

pub use attr::Attr;
pub use combine::Combine;
pub use list::List;
pub use next::Next;
pub use text::Text;
pub use url::Url;

#[derive(Debug, PartialEq)]
pub enum Value {
    /// 空结果
    Empty,
    /// 单值结果
    Single(String),
    /// 多值结果
    Multiple(Vec<String>),
}

#[typetag::deserialize(tag = "type")]
pub trait Extractor: Send + Sync {
    fn extract(&self, element: ElementRef) -> Value;

    fn extract_all(&self, element: ElementRef) -> Value;

    // fn iter<'a>(&self, element: ElementRef<'a>) -> Select<'a, '_>;
}

#[derive(Deserialize)]
pub struct ContentExtractor {
    #[serde(deserialize_with = "deserialize_selector")]
    pub this: Selector,
    pub paragraphs: Box<dyn Extractor>,
    pub next_url: Option<Box<dyn Extractor>>,
    #[serde(default = "default_title_pattern")]
    pub title_pattern: String,
    pub title: Option<Box<dyn Extractor>>,
}

fn default_title_pattern() -> String {
    r#"^体育祭开幕(（\d+/\d+）)?$"#.to_string()
}

impl ContentExtractor {
    pub fn extract_paragraphs<'a>(&self, this: ElementRef<'a>) -> Value {
        self.paragraphs.extract(this)
    }

    pub fn extract_next_url<'a>(&self, this: ElementRef<'a>) -> Value {
        match &self.next_url {
            Some(extractor) => extractor.extract(this),
            None => Value::Empty,
        }
    }

    pub fn matches_title(&self, title: &str, target: &str) -> bool {
        let pattern = self.title_pattern.replace("{title}", title);

        Regex::new(&pattern)
            .expect("正则表达式编译失败")
            .is_match(target)
    }

    pub fn extract_title<'a>(&self, this: ElementRef<'a>) -> Value {
        match &self.title {
            Some(extractor) => extractor.extract(this),
            None => Value::Empty,
        }
    }
}

// #[derive(Deserialize)]
// pub struct ChapterExtractor {
//     #[serde(deserialize_with = "deserialize_selector")]
//     pub this: Selector,
//     pub title: Box<dyn Extractor>,
//     pub content_url: Box<dyn Extractor>,
//     pub paragraphs: Box<dyn Extractor>,
// }

#[derive(Deserialize)]
pub struct ChapterExtractor {
    #[serde(deserialize_with = "deserialize_selector")]
    pub this: Selector,
    pub title: Box<dyn Extractor>,
    pub content_url: Box<dyn Extractor>,
    pub content: ContentExtractor,
}

impl ChapterExtractor {
    pub fn extract_title(&self, this: ElementRef) -> Value {
        self.title.extract(this)
    }

    pub fn extract_content_url(&self, this: ElementRef) -> Value {
        self.content_url.extract(this)
    }

    // pub fn extract_paragraphs(&self, this: ElementRef) -> Value {
    //     self.paragraphs.extract(this)
    // }
}

#[derive(Deserialize)]
pub struct VolumeExtractor {
    #[serde(deserialize_with = "deserialize_selector")]
    pub this: Selector,
    pub title: Box<dyn Extractor>,
    pub cover_url: Option<Box<dyn Extractor>>,
    pub chapters: ChapterExtractor,
}

impl VolumeExtractor {
    pub fn extract_title(&self, this: ElementRef) -> Value {
        self.title.extract(this)
    }

    pub fn extract_cover_url(&self, this: ElementRef) -> Value {
        match &self.cover_url {
            Some(cover_extractor) => cover_extractor.extract(this),
            None => Value::Empty,
        }
    }

    pub fn chapter_iter<'a>(&self, this: ElementRef<'a>) -> Select<'a, '_> {
        this.select(&self.chapters.this)
    }
}

#[derive(Deserialize)]
pub struct BookExtractor {
    #[serde(deserialize_with = "deserialize_selector")]
    pub this: Selector,
    pub title: Box<dyn Extractor>,
    pub author: Box<dyn Extractor>,
    pub illustrator: Option<Box<dyn Extractor>>,
    pub tags: Option<Box<dyn Extractor>>,
    pub summary: Option<Box<dyn Extractor>>,
    pub cover_url: Option<Box<dyn Extractor>>,
    pub volumes: Option<VolumeExtractor>,
    pub chapters: Option<ChapterExtractor>,
}

impl BookExtractor {
    pub fn this<'a>(&self, element: ElementRef<'a>) -> Option<ElementRef<'a>> {
        element.select(&self.this).next()
    }

    pub fn extract_title(&self, this: ElementRef) -> Value {
        self.title.extract(this)
    }

    pub fn extract_author(&self, this: ElementRef) -> Value {
        self.author.extract(this)
    }

    pub fn extract_illustrator(&self, this: ElementRef) -> Value {
        match &self.illustrator {
            Some(illust_extractor) => illust_extractor.extract(this),
            None => Value::Empty,
        }
    }

    pub fn extract_tags(&self, this: ElementRef) -> Value {
        match &self.tags {
            Some(tags_extractor) => tags_extractor.extract(this),
            None => Value::Empty,
        }
    }

    pub fn extract_summary(&self, this: ElementRef) -> Value {
        match &self.summary {
            Some(summary_extractor) => summary_extractor.extract(this),
            None => Value::Empty,
        }
    }

    pub fn extract_cover_url(&self, this: ElementRef) -> Value {
        match &self.cover_url {
            Some(cover_extractor) => cover_extractor.extract(this),
            None => Value::Empty,
        }
    }
}

fn deserialize_selector<'de, D>(deserializer: D) -> Result<Selector, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    Selector::parse(&s).map_err(|e| serde::de::Error::custom(format!("Invalid selector: {}", e)))
}

fn deserialize_nullable_selector<'de, D>(deserializer: D) -> Result<Option<Selector>, D::Error>
where
    D: Deserializer<'de>,
{
    let option_str: Option<String> = Option::deserialize(deserializer)?;

    match option_str {
        Some(s) if s.trim().is_empty() => Ok(None), // 空字符串也视为 None
        Some(s) => Selector::parse(&s)
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid selector '{}': {}", s, e))),
        None => Ok(None),
    }
}
