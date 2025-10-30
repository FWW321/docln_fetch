use scraper::ElementRef;
use serde::Deserialize;

use super::{Extractor, List, Value};

#[derive(Deserialize)]
pub struct Combine {
    separator: String,
    items: List,
}

#[typetag::deserialize]
impl Extractor for Combine {
    fn extract(&self, element: ElementRef) -> Value {
        let mut combined = Vec::new();

        let value = self.items.extract(element);
        match value {
            Value::Single(v) => combined.push(v),
            Value::Multiple(vs) => combined.extend(vs),
            Value::Empty => (),
        }

        if combined.is_empty() {
            Value::Empty
        } else {
            Value::Single(combined.join(&self.separator))
        }
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut combined = Vec::new();

        let value = self.items.extract_all(element);
        match value {
            Value::Single(v) => combined.push(v),
            Value::Multiple(vs) => combined.extend(vs),
            Value::Empty => (),
        }

        if combined.is_empty() {
            Value::Empty
        } else {
            Value::Single(combined.join(&self.separator))
        }
    }
}
