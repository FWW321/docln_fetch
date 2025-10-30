use scraper::{ElementRef, Selector};
use serde::Deserialize;

use super::{Extractor, Value, deserialize_selector};

#[derive(Deserialize)]
pub struct Html {
    #[serde(deserialize_with = "deserialize_selector")]
    selector: Selector,
}

#[typetag::deserialize]
impl Extractor for Html {
    fn extract(&self, element: ElementRef) -> Value {
        let html = element.select(&self.selector).next().map(|e| e.html());
        html.map_or(Value::Empty, Value::Single)
    }

    fn extract_all(&self, element: ElementRef) -> Value {
        let mut results = Vec::new();

        for elem in element.select(&self.selector) {
            let html = elem.html();
            results.push(html);
        }

        if results.is_empty() {
            Value::Empty
        } else {
            Value::Multiple(results)
        }
    }
}
