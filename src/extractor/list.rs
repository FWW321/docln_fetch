use scraper::{ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_selector};

#[derive(Deserialize)]
pub struct List {
    #[serde(deserialize_with = "deserialize_selector")]
    selector: Selector,
    item: Box<dyn Extractor>,
}

#[typetag::deserialize]
impl Extractor for List {
    fn extract(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        let Some(container) = element.select(&self.selector).next() else {
            return Value::Empty;
        };

        let value = self.item.extract_all(container);

        match value {
            Value::Single(v) => results.push(v),
            Value::Multiple(vs) => results.extend(vs),
            Value::Empty => (),
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        for container in element.select(&self.selector) {
            let value = self.item.extract_all(container);
            match value {
                Value::Single(v) => results.push(v),
                Value::Multiple(vs) => results.extend(vs),
                Value::Empty => (),
            }
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }
}
