use scraper::{ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_nullable_selector};

#[derive(Deserialize)]
pub struct Attr {
    #[serde(default, deserialize_with = "deserialize_nullable_selector")]
    pub selector: Option<Selector>,
    pub name: String,
}

#[typetag::deserialize]
impl Extractor for Attr {
    fn extract(&self, element: ElementRef) -> Value {
        let element = if let Some(selector) = &self.selector {
            element.select(selector).next()
        } else {
            Some(element)
        };
        let attr = element.and_then(|e| e.value().attr(&self.name));

        attr.map_or(Value::Empty, |v| Value::Single(v.to_string()))
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        if let Some(selector) = &self.selector {
            for elem in element.select(selector) {
                if let Some(attr) = elem.value().attr(&self.name) {
                    results.push(attr.to_string());
                }
            }
        } else {
            if let Some(attr) = element.value().attr(&self.name) {
                results.push(attr.to_string());
            }
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }
}
