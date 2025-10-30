use scraper::{ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_nullable_selector};

#[derive(Debug, Deserialize)]
pub struct Text {
    #[serde(default, deserialize_with = "deserialize_nullable_selector")]
    selector: Option<Selector>,
}

#[typetag::deserialize]
impl Extractor for Text {
    fn extract(&self, element: ElementRef) -> Value {
        let elem = if let Some(selector) = &self.selector {
            element.select(selector).next()
        } else {
            Some(element)
        };
        if let Some(elem) = elem {
            let text = elem.text().collect::<String>();
            if text.is_empty() {
                Value::Empty
            } else {
                Value::Single(text)
            }
        } else {
            Value::Empty
        }
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        if let Some(selector) = &self.selector {
            for elem in element.select(selector) {
                let text = elem.text().collect::<String>();
                results.push(text);
            }
        } else {
            let text = element.text().collect::<String>();
            results.push(text);
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }
}
