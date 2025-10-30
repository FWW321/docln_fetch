use scraper::{ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_selector};

#[derive(Deserialize)]
pub struct Current {
    #[serde(deserialize_with = "deserialize_selector")]
    base: Selector,
    condition: Option<String>,
    current: Box<dyn Extractor>,
}

#[typetag::deserialize]
impl Extractor for Current {
    fn extract(&self, element: ElementRef) -> Value {
        for base_elem in element.select(&self.base) {
            if let Some(cond) = &self.condition {
                if !base_elem.text().any(|t| t.contains(cond)) {
                    continue;
                }
            }

            return self.current.extract(base_elem);
        }
        Value::Empty
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        for base_elem in element.select(&self.base) {
            if let Some(cond) = &self.condition {
                if !base_elem.text().any(|t| t.contains(cond)) {
                    continue;
                }
            }

            match self.current.extract(base_elem) {
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
