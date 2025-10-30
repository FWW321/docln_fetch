use scraper::{Element, ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_selector};

#[derive(Deserialize)]
pub struct Next {
    #[serde(deserialize_with = "deserialize_selector")]
    current: Selector,
    condition: Option<String>,
    next: Box<dyn Extractor>,
}

#[typetag::deserialize]
impl Extractor for Next {
    fn extract(&self, element: ElementRef) -> Value {
        for base_elem in element.select(&self.current) {
            if let Some(cond) = &self.condition {
                if !base_elem.text().any(|t| t.contains(cond)) {
                    continue;
                }
            }

            if let Some(sibling_elem) = base_elem.next_sibling_element() {
                return self.next.extract(sibling_elem);
            }
        }
        Value::Empty
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        for base_elem in element.select(&self.current) {
            if let Some(cond) = &self.condition {
                if !base_elem.text().any(|t| t.contains(cond)) {
                    continue;
                }
            }

            if let Some(sibling) = base_elem.next_sibling() {
                if let Some(sibling_elem) = ElementRef::wrap(sibling) {
                    match self.next.extract(sibling_elem) {
                        Value::Single(v) => results.push(v),
                        Value::Multiple(vs) => results.extend(vs),
                        Value::Empty => (),
                    }
                }
            }
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }
}
