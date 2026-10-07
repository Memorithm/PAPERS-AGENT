use std::path::Path;

use log::info;
use serde::{Deserialize, Serialize};

use crate::paper_parser::{PaperParser, ParsedPaper};

/// Information extraite d'une source (PDF, arXiv, URL, texte).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedDocument {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub publication_date: Option<String>,
    pub source: String,
    pub paper_url: Option<String>,
    pub github_url: Option<String>,
    pub abstract_text: Option<String>,
    pub full_text: Option<String>,
    pub references: Vec<String>,
    pub parsed: Option<ParsedPaper>,
}

/// Trait pour les extracteurs de différentes sources.
pub trait SourceExtractor: Send + Sync {
    fn can_handle(&self, source: &str) -> bool;
    fn extract(&self, source: &str) -> Result<ExtractedDocument, String>;
    fn name(&self) -> &str;
}

// ── Extracteur arXiv ────────────────────────────────────────────

pub struct ArxivExtractor {
    client: Result<reqwest::blocking::Client, String>,
}

impl Default for ArxivExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ArxivExtractor {
    pub fn new() -> Self {
        Self {
            client: crate::arxiv_http::client(),
        }
    }

    fn extract_arxiv_id(source: &str) -> Option<String> {
        use regex::Regex;
        if let Ok(re) = Regex::new(r"arxiv\.org/abs/(\d+\.\d+)") {
            if let Some(caps) = re.captures(source) {
                return caps.get(1).map(|m| m.as_str().to_string());
            }
        }
        if let Ok(re) = Regex::new(r"^(\d{4}\.\d{4,5})$") {
            if let Some(caps) = re.captures(source) {
                return caps.get(1).map(|m| m.as_str().to_string());
            }
        }
        None
    }
}

impl SourceExtractor for ArxivExtractor {
    fn can_handle(&self, source: &str) -> bool {
        source.contains("arxiv.org")
            || regex::Regex::new(r"^\d{4}\.\d{4,5}$")
                .map(|re| re.is_match(source))
                .unwrap_or(false)
    }

    fn extract(&self, source: &str) -> Result<ExtractedDocument, String> {
        let paper_id = Self::extract_arxiv_id(source)
            .ok_or_else(|| format!("ID arXiv non reconnu: {}", source))?;

        info!("Extraction arXiv: {}", paper_id);

        let url = format!(
            "{}?search_query=id:{}&max_results=1",
            crate::arxiv_http::API_URL,
            paper_id
        );
        let resp = self
            .client
            .as_ref()
            .map_err(Clone::clone)?
            .get(&url)
            .send()
            .map_err(|e| format!("Erreur HTTP arXiv: {}", e))?;
        let body = crate::arxiv_http::read_response(resp)?;

        // Parsing XML basique de la réponse arXiv API
        let title = extract_xml_tag(&body, "title").unwrap_or_else(|| paper_id.clone());
        let summary = extract_xml_tag(&body, "summary");
        let published = extract_xml_tag(&body, "published");
        let authors = extract_xml_tags(&body, "author", "name");
        let pdf_url = body
            .lines()
            .find(|l| l.contains("title=\"pdf\""))
            .and_then(|l| {
                let start = l.find("href=\"")? + 6;
                let end = l[start..].find('"')? + start;
                Some(l[start..end].to_string())
            });

        Ok(ExtractedDocument {
            id: format!("ARXIV-{}", paper_id.replace('.', "-")),
            title,
            authors,
            publication_date: published,
            source: "arXiv".to_string(),
            paper_url: pdf_url,
            github_url: None,
            abstract_text: summary,
            full_text: None,
            references: Vec::new(),
            parsed: None,
        })
    }

    fn name(&self) -> &str {
        "arxiv"
    }
}

/// Recherche les derniers papiers arXiv correspondant à une requête libre.
///
/// Utilise l'API Atom d'arXiv (`search_query=all:{query}`) triée par date de
/// soumission décroissante. Retourne au plus `max_results` documents.
pub fn search_arxiv_latest(
    query: &str,
    max_results: usize,
) -> Result<Vec<ExtractedDocument>, String> {
    let client = crate::arxiv_http::client()?;
    let url = format!(
        "{}?search_query=all:{}&sortBy=submittedDate&sortOrder=descending&max_results={}",
        crate::arxiv_http::API_URL,
        urlencode(query),
        max_results
    );
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Erreur HTTP arXiv: {e}"))?;
    let body = crate::arxiv_http::read_response(response)?;

    Ok(parse_arxiv_feed(&body))
}

/// Parse un flux Atom arXiv en documents (un par `<entry>`).
fn parse_arxiv_feed(body: &str) -> Vec<ExtractedDocument> {
    let mut docs = Vec::new();
    // Chaque papier est contenu dans un bloc <entry>…</entry>.
    for chunk in body.split("</entry>") {
        let entry = match chunk.find("<entry>") {
            Some(pos) => &chunk[pos..],
            None => continue,
        };
        let id_url = extract_xml_tag(entry, "id").unwrap_or_default();
        let paper_id = id_url.rsplit('/').next().unwrap_or("").to_string();
        if paper_id.is_empty() {
            continue;
        }
        docs.push(ExtractedDocument {
            id: format!("ARXIV-{}", paper_id.replace('.', "-")),
            title: extract_xml_tag(entry, "title").unwrap_or_else(|| paper_id.clone()),
            authors: extract_xml_tags(entry, "author", "name"),
            publication_date: extract_xml_tag(entry, "published"),
            source: "arXiv".into(),
            paper_url: Some(format!("https://arxiv.org/abs/{paper_id}")),
            github_url: None,
            abstract_text: extract_xml_tag(entry, "summary"),
            full_text: None,
            references: Vec::new(),
            parsed: None,
        });
    }
    docs
}

/// Encodage minimal pour paramètre de requête (espaces + non-alphanumériques).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                for byte in ch.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

// ── Extracteur PDF ──────────────────────────────────────────────

pub struct PdfExtractor;

impl Default for PdfExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl SourceExtractor for PdfExtractor {
    fn can_handle(&self, source: &str) -> bool {
        let lower = source.to_lowercase();
        if !lower.ends_with(".pdf") {
            return false;
        }
        // Vérifier que ce n'est pas une URL http/https
        !lower.starts_with("http://") && !lower.starts_with("https://")
    }

    fn extract(&self, source: &str) -> Result<ExtractedDocument, String> {
        info!("Extraction PDF: {}", source);

        let path = Path::new(source);
        if !path.exists() {
            return Err(format!("Fichier PDF non trouvé: {}", source));
        }

        let text =
            pdf_extract::extract_text(path).map_err(|e| format!("Erreur extraction PDF: {}", e))?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let parsed = PaperParser::parse(&text, Some(stem));
        let title = parsed.title.clone().unwrap_or_else(|| stem.to_string());
        let abstract_text = parsed.abstract_text.clone().or_else(|| {
            if text.len() > 2000 {
                Some(text[..2000].to_string())
            } else {
                Some(text.clone())
            }
        });
        let github = parsed.github_urls.first().cloned();

        Ok(ExtractedDocument {
            id: format!("PDF-{}", stem.to_uppercase()),
            title,
            authors: Vec::new(),
            publication_date: None,
            source: format!("PDF local: {}", source),
            paper_url: None,
            github_url: github,
            abstract_text,
            full_text: Some(text),
            references: parsed.references.clone(),
            parsed: Some(parsed),
        })
    }

    fn name(&self) -> &str {
        "pdf"
    }
}

// ── Extracteur Texte brut ───────────────────────────────────────

pub struct PlainTextExtractor;

impl Default for PlainTextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl PlainTextExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl SourceExtractor for PlainTextExtractor {
    fn can_handle(&self, source: &str) -> bool {
        let lower = source.to_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            lower.ends_with(".txt") || lower.ends_with(".md")
        } else {
            false
        }
    }

    fn extract(&self, source: &str) -> Result<ExtractedDocument, String> {
        info!("Extraction texte: {}", source);

        let path = Path::new(source);
        if !path.exists() {
            return Err(format!("Fichier non trouvé: {}", source));
        }

        let text =
            std::fs::read_to_string(path).map_err(|e| format!("Erreur lecture fichier: {}", e))?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let parsed = PaperParser::parse(&text, Some(stem));
        let title = parsed.title.clone().unwrap_or_else(|| stem.to_string());
        let abstract_text = parsed.abstract_text.clone().or_else(|| {
            if text.len() > 2000 {
                Some(text[..2000].to_string())
            } else {
                Some(text.clone())
            }
        });
        let github = parsed.github_urls.first().cloned();

        Ok(ExtractedDocument {
            id: format!("TEXT-{}", stem.to_uppercase()),
            title,
            authors: Vec::new(),
            publication_date: None,
            source: format!("Texte local: {}", source),
            paper_url: None,
            github_url: github,
            abstract_text,
            full_text: Some(text),
            references: parsed.references.clone(),
            parsed: Some(parsed),
        })
    }

    fn name(&self) -> &str {
        "text"
    }
}

// ── Extracteur URL ──────────────────────────────────────────────

pub struct UrlExtractor {
    client: reqwest::blocking::Client,
}

impl Default for UrlExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl UrlExtractor {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl SourceExtractor for UrlExtractor {
    fn can_handle(&self, source: &str) -> bool {
        source.starts_with("http://") || source.starts_with("https://")
    }

    fn extract(&self, source: &str) -> Result<ExtractedDocument, String> {
        info!("Extraction URL: {}", source);

        let resp = self
            .client
            .get(source)
            .send()
            .map_err(|e| format!("Erreur HTTP: {}", e))?;
        let text = resp
            .text()
            .map_err(|e| format!("Erreur lecture réponse: {}", e))?;

        let abstract_text = if text.len() > 2000 {
            Some(text[..2000].to_string())
        } else {
            Some(text.clone())
        };

        Ok(ExtractedDocument {
            id: format!("URL-{:08x}", seahash::hash(source.as_bytes()) as u32),
            title: source.to_string(),
            authors: Vec::new(),
            publication_date: None,
            source: source.to_string(),
            paper_url: Some(source.to_string()),
            github_url: None,
            abstract_text,
            full_text: Some(text),
            references: Vec::new(),
            parsed: None,
        })
    }

    fn name(&self) -> &str {
        "url"
    }
}

// ── Pipeline d'extraction ───────────────────────────────────────

pub struct ExtractionPipeline {
    extractors: Vec<Box<dyn SourceExtractor>>,
}

impl ExtractionPipeline {
    pub fn new() -> Self {
        Self {
            extractors: vec![
                Box::new(ArxivExtractor::new()),
                Box::new(PdfExtractor::new()),
                Box::new(PlainTextExtractor::new()),
                Box::new(UrlExtractor::new()),
            ],
        }
    }

    pub fn extract(&self, source: &str) -> Result<ExtractedDocument, String> {
        for ext in &self.extractors {
            if ext.can_handle(source) {
                return ext.extract(source);
            }
        }
        Err(format!("Aucun extracteur ne peut traiter: {}", source))
    }
}

impl Default for ExtractionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers XML (pour arXiv) ────────────────────────────────────

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let content = &xml[start..end];
    // Nettoyer les espaces et retours à la ligne
    let cleaned = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn extract_xml_tags(xml: &str, parent: &str, child: &str) -> Vec<String> {
    let mut results = Vec::new();
    let parent_open = format!("<{}>", parent);
    let parent_close = format!("</{}>", parent);
    let mut rest = xml;
    while let Some(p_start) = rest.find(&parent_open) {
        let after_open = p_start + parent_open.len();
        let remaining = &rest[after_open..];
        let p_end = remaining.find(&parent_close).unwrap_or(remaining.len());
        let block = &remaining[..p_end];
        if let Some(val) = extract_xml_tag(block, child) {
            results.push(val);
        }
        rest = &remaining[p_end + parent_close.len()..];
    }
    results
}

// Pas de dépendance externe pour le hash, on utilise un simple hash
mod seahash {
    pub fn hash(data: &[u8]) -> u64 {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(data);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_pipeline_unknown() {
        let pipeline = ExtractionPipeline::new();
        assert!(pipeline.extract("blah.invalid").is_err());
    }

    #[test]
    fn test_pdf_can_handle() {
        let ext = PdfExtractor::new();
        assert!(ext.can_handle("/path/to/paper.pdf"));
        assert!(!ext.can_handle("https://example.com/paper.pdf"));
        assert!(!ext.can_handle("paper.txt"));
    }

    #[test]
    fn test_arxiv_can_handle() {
        let ext = ArxivExtractor::new();
        assert!(ext.can_handle("https://arxiv.org/abs/2401.00001"));
        assert!(ext.can_handle("2401.00001"));
        assert!(!ext.can_handle("paper.pdf"));
    }

    #[test]
    fn test_parse_arxiv_feed_multiple_entries() {
        let feed = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>ArXiv Query</title>
  <entry>
    <id>http://arxiv.org/abs/2608.01234v1</id>
    <updated>2026-08-20T18:00:00Z</updated>
    <published>2026-08-20T18:00:00Z</published>
    <title>First Paper About
      Memory</title>
    <summary>Abstract of first paper.</summary>
    <author><name>Alice Author</name></author>
    <author><name>Bob Builder</name></author>
  </entry>
  <entry>
    <id>http://arxiv.org/abs/2608.05678v2</id>
    <published>2026-08-21T09:00:00Z</published>
    <title>Second Paper</title>
    <summary>Abstract of second paper.</summary>
    <author><name>Carol Chen</name></author>
  </entry>
</feed>"#;

        let docs = parse_arxiv_feed(feed);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].id, "ARXIV-2608-01234v1");
        assert_eq!(docs[0].title, "First Paper About Memory");
        assert_eq!(docs[0].authors.len(), 2);
        assert_eq!(docs[0].authors[0], "Alice Author");
        assert_eq!(docs[1].id, "ARXIV-2608-05678v2");
        assert_eq!(
            docs[1].paper_url.as_deref(),
            Some("https://arxiv.org/abs/2608.05678v2")
        );

        // Flux vide ou sans entry → aucun document.
        assert!(parse_arxiv_feed("<feed></feed>").is_empty());
    }

    #[test]
    fn test_urlencode_spaces_and_specials() {
        assert_eq!(
            urlencode("reinforcement learning"),
            "reinforcement%20learning"
        );
        assert_eq!(urlencode("a&b"), "a%26b");
        assert_eq!(urlencode("safe-token_1.0"), "safe-token_1.0");
    }

    #[test]
    fn test_text_can_handle() {
        let ext = PlainTextExtractor::new();
        assert!(ext.can_handle("/path/to/file.txt"));
        assert!(ext.can_handle("notes.md"));
        assert!(!ext.can_handle("paper.pdf"));
        assert!(!ext.can_handle("https://example.com/file.txt"));
    }

    #[test]
    fn test_url_can_handle() {
        let ext = UrlExtractor::new();
        assert!(ext.can_handle("https://example.com/paper"));
        assert!(ext.can_handle("http://arxiv.org/abs/2401.00001"));
        assert!(!ext.can_handle("paper.pdf"));
    }

    #[test]
    fn test_extract_xml_tag() {
        let xml = "<entry><title>Test Title</title><summary>Abstract here</summary></entry>";
        assert_eq!(
            extract_xml_tag(xml, "title"),
            Some("Test Title".to_string())
        );
        assert_eq!(
            extract_xml_tag(xml, "summary"),
            Some("Abstract here".to_string())
        );
        assert_eq!(extract_xml_tag(xml, "nonexistent"), None);
    }
}
