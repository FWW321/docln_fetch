use linkify::LinkFinder;
use scraper::ElementRef;
use serde::Deserialize;

use super::{Attr, Extractor, Value};

#[derive(Deserialize)]
pub struct Url {
    inner: Attr,
}

#[typetag::deserialize]
impl Extractor for Url {
    fn extract(&self, element: ElementRef) -> Value {
        let value = self.inner.extract(element);
        if self.inner.name == "href" || self.inner.name == "src" {
            return value;
        }
        let mut finder = LinkFinder::new();
        finder.url_must_have_scheme(false);
        let mut urls = Vec::new();

        match value {
            Value::Single(text) => {
                urls.extend(finder.links(&text).map(|l| l.as_str().to_string()));
            }
            Value::Multiple(texts) => {
                for text in texts {
                    urls.extend(finder.links(&text).map(|l| l.as_str().to_string()));
                }
            }
            Value::Empty => (),
        }

        match urls.len() {
            0 => Value::Empty,
            1 => Value::Single(urls.into_iter().next().unwrap()),
            _ => Value::Multiple(urls),
        }
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let value = self.inner.extract_all(element);
        let mut finder = LinkFinder::new();
        finder.url_must_have_scheme(false);
        let mut urls = Vec::new();

        match value {
            Value::Single(text) => {
                urls.extend(finder.links(&text).map(|l| l.as_str().to_string()));
            }
            Value::Multiple(texts) => {
                for text in texts {
                    urls.extend(finder.links(&text).map(|l| l.as_str().to_string()));
                }
            }
            Value::Empty => (),
        }

        if urls.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(urls)
        }
    }
}
