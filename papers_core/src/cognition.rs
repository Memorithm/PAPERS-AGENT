use crate::models::CognitionItem;

#[derive(Debug, Clone)]
pub struct CognitionStore {
    items: Vec<CognitionItem>,
}

impl CognitionStore {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: CognitionItem) {
        self.items.push(item);
    }

    pub fn add_batch(&mut self, items: Vec<CognitionItem>) {
        self.items.extend(items);
    }

    pub fn retrieve(&self, query: &str, n: usize) -> Vec<&CognitionItem> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut scored: Vec<(usize, f64)> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let content_lower = item.content.to_lowercase();
                let score = if content_lower.contains(&query_lower) {
                    0.9
                } else {
                    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
                    let matches = query_words
                        .iter()
                        .filter(|w| content_lower.contains(*w))
                        .count();
                    if query_words.is_empty() {
                        0.0
                    } else {
                        matches as f64 / query_words.len() as f64 * 0.5
                    }
                };
                (i, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.total_cmp(&a.1));

        scored
            .into_iter()
            .take(n)
            .map(|(i, _)| &self.items[i])
            .collect()
    }

    /// Retrieve all items regardless of query (for seed command).
    pub fn retrieve_all(&self) -> Vec<&CognitionItem> {
        self.items.iter().collect()
    }

    pub fn search_by_tag(&self, tag: &str) -> Vec<&CognitionItem> {
        self.items
            .iter()
            .filter(|item| item.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Default for CognitionStore {
    fn default() -> Self {
        Self::new()
    }
}
