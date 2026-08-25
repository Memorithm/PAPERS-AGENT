use regex::Regex;
use serde::{Deserialize, Serialize};

/// Résultat du parsing sémantique d'un papier académique.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedPaper {
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub sections: Vec<(String, String)>,
    pub equations: Vec<String>,
    pub variables: Vec<VariableDef>,
    pub datasets: Vec<String>,
    pub metrics: Vec<String>,
    pub limitations: Vec<String>,
    pub github_urls: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDef {
    pub name: String,
    pub meaning: String,
}

/// Parser sémantique de papiers académiques basé sur des regex.
///
/// Portage Rust du `SemanticPaperParser` Python.
pub struct PaperParser;

impl PaperParser {
    pub fn parse(text: &str, title: Option<&str>) -> ParsedPaper {
        let sections = Self::extract_sections(text);
        let abstract_text = sections
            .iter()
            .find(|(name, _)| name == "abstract")
            .map(|(_, t)| t.clone())
            .or_else(|| Self::extract_abstract(text));

        ParsedPaper {
            title: title.map(String::from),
            abstract_text,
            sections,
            equations: Self::extract_equations(text),
            variables: Self::extract_variables(text),
            datasets: Self::extract_datasets(text),
            metrics: Self::extract_metrics(text),
            limitations: Self::extract_limitations(text),
            github_urls: Self::extract_github_urls(text),
            references: Self::extract_references(text),
        }
    }

    fn extract_sections(text: &str) -> Vec<(String, String)> {
        let patterns: &[(&str, &str)] = &[
            (r"(?im)^\s*(abstract|résumé)\s*$", "abstract"),
            (r"(?im)^\s*(introduction)\s*$", "introduction"),
            (
                r"(?im)^\s*(related work|état de l'art|travaux connexes)\s*$",
                "related_work",
            ),
            (
                r"(?im)^\s*(method|methods|méthode|méthodes|methodology)\s*$",
                "method",
            ),
            (
                r"(?im)^\s*(experiments|expériences|experimental setup)\s*$",
                "experiments",
            ),
            (r"(?im)^\s*(results|résultats)\s*$", "results"),
            (r"(?im)^\s*(discussion)\s*$", "discussion"),
            (r"(?im)^\s*(conclusion|conclusions)\s*$", "conclusion"),
            (r"(?im)^\s*(limitations|limites)\s*$", "limitations"),
            (r"(?im)^\s*(references|références)\s*$", "references"),
        ];

        let mut matches: Vec<(usize, &str)> = Vec::new();
        for (pattern, name) in patterns {
            if let Ok(re) = Regex::new(pattern) {
                for m in re.find_iter(text) {
                    matches.push((m.start(), *name));
                }
            }
        }
        matches.sort_by_key(|(pos, _)| *pos);

        let mut sections: Vec<(String, String)> = Vec::new();
        for i in 0..matches.len() {
            let (start, name) = matches[i];
            let end = if i + 1 < matches.len() {
                matches[i + 1].0
            } else {
                text.len()
            };
            sections.push((name.to_string(), text[start..end].trim().to_string()));
        }
        sections
    }

    fn extract_abstract(text: &str) -> Option<String> {
        let re = Regex::new(r"(?is)abstract[\s:]*(.{100,2000})\n\s*\n").ok()?;
        re.captures(text)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
    }

    fn extract_equations(text: &str) -> Vec<String> {
        let inline = Regex::new(r"\$[^$]+?\$")
            .ok()
            .map(|re| {
                re.find_iter(text)
                    .map(|m| m.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let display = Regex::new(r"\$\$[^$]+?\$\$")
            .ok()
            .map(|re| {
                re.find_iter(text)
                    .map(|m| m.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut all: Vec<String> = inline.into_iter().chain(display).collect();
        all.sort();
        all.dedup();
        all.truncate(50);
        all
    }

    fn extract_variables(text: &str) -> Vec<VariableDef> {
        let re = match Regex::new(r"([A-Za-z][A-Za-z0-9_]*)\s*=\s*([^\n,;]+)") {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut seen = std::collections::HashSet::new();
        let mut vars = Vec::new();
        for caps in re.captures_iter(text) {
            let name = caps
                .get(1)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            if name.len() <= 1 || seen.contains(&name) {
                continue;
            }
            seen.insert(name.clone());
            vars.push(VariableDef {
                name,
                meaning: caps
                    .get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
            });
        }
        vars.truncate(30);
        vars
    }

    fn extract_datasets(text: &str) -> Vec<String> {
        let keywords = [
            "dataset",
            "datasets",
            "benchmark",
            "corpus",
            "pg19",
            "proof-pile",
            "c4",
            "the pile",
            "pile",
            "wikitext",
            "hellaswag",
            "mmlu",
            "gsm8k",
            "swag",
            "squad",
            "glue",
            "superglue",
            "enwik8",
            "lambada",
            "arc",
            "boolq",
            "piqa",
            "winogrande",
            "openbookqa",
        ];

        let mut found: Vec<String> = Vec::new();
        for kw in &keywords {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(kw));
            if let Ok(re) = Regex::new(&pattern) {
                for m in re.find_iter(text) {
                    let start = m.start().saturating_sub(30);
                    let end = (m.end() + 30).min(text.len());
                    found.push(text[start..end].trim().to_string());
                }
            }
        }
        found.sort();
        found.dedup();
        found.truncate(20);
        found
    }

    fn extract_metrics(text: &str) -> Vec<String> {
        let keywords = [
            "perplexity",
            "bleu",
            "rouge",
            "accuracy",
            "f1",
            "precision",
            "recall",
            "latency",
            "throughput",
            "flops",
            "params",
            "memory",
            "vram",
            "perplexité",
            "exact match",
            "em",
            "mrr",
            "ndcg",
            "map",
        ];

        let mut found: Vec<String> = Vec::new();
        for kw in &keywords {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(kw));
            if let Ok(re) = Regex::new(&pattern) {
                for m in re.find_iter(text) {
                    let start = m.start().saturating_sub(25);
                    let end = (m.end() + 40).min(text.len());
                    found.push(text[start..end].trim().to_string());
                }
            }
        }
        found.sort();
        found.dedup();
        found.truncate(20);
        found
    }

    fn extract_limitations(text: &str) -> Vec<String> {
        let header_re = match Regex::new(r"(?is)(?:limitations|limites|weaknesses)\s*[\n:]") {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let header = match header_re.find(text) {
            Some(h) => h,
            None => return Vec::new(),
        };
        let content_start = header.end();

        let section_re = match Regex::new(
            r"(?im)^\s*(?:references|références|conclusion|conclusions|appendix|acknowledgements)\s*$",
        ) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let remaining = &text[content_start..];
        let content_end = section_re
            .find(remaining)
            .map(|m| content_start + m.start())
            .unwrap_or(text.len());

        let content = &text[content_start..content_end];
        content
            .split(|c| ['.', '!', '?'].contains(&c))
            .map(|s| s.trim())
            .filter(|s| s.len() > 20)
            .take(10)
            .map(String::from)
            .collect()
    }

    fn extract_github_urls(text: &str) -> Vec<String> {
        let re = Regex::new(r"https?://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/?");
        re.ok()
            .map(|re| {
                re.find_iter(text)
                    .map(|m| m.as_str().trim_end_matches('/').to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn extract_references(text: &str) -> Vec<String> {
        let marker_re = match Regex::new(r"\[\d+\]") {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let positions: Vec<usize> = marker_re.find_iter(text).map(|m| m.start()).collect();
        if positions.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        for i in 0..positions.len() {
            let start = positions[i];
            let end = if i + 1 < positions.len() {
                positions[i + 1]
            } else {
                text.len()
            };
            let block = &text[start..end];
            if let Some(bracket_end) = block.find(']') {
                let ref_text = block[bracket_end + 1..].trim().replace('\n', " ");
                if !ref_text.is_empty() {
                    results.push(ref_text);
                }
            }
        }
        results.truncate(50);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_abstract() {
        let text = "Abstract\nThis is a test paper about machine learning. It explores various topics.\n\nIntroduction\nThe field has grown rapidly.";
        let parsed = PaperParser::parse(text, Some("Test Paper"));
        assert_eq!(parsed.title.as_deref(), Some("Test Paper"));
        assert!(parsed.sections.iter().any(|(n, _)| n == "abstract"));
    }

    #[test]
    fn test_extract_github_urls() {
        let text =
            "Code available at https://github.com/user/repo and docs at https://github.com/org/lib";
        let urls = PaperParser::extract_github_urls(text);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://github.com/user/repo".to_string()));
    }

    #[test]
    fn test_extract_equations() {
        let text = r"The formula $E = mc^2$ and $$\sum_{i=1}^n x_i$$ are key.";
        let eqs = PaperParser::extract_equations(text);
        assert!(!eqs.is_empty());
    }

    #[test]
    fn test_empty_text() {
        let parsed = PaperParser::parse("", None);
        assert!(parsed.sections.is_empty());
        assert!(parsed.equations.is_empty());
    }
}
