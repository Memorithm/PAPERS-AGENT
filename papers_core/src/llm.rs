//! Client LLM unifié (Ollama + API OpenAI-compatible).
//!
//! - Timeouts configurables sur toutes les requêtes.
//! - Retry avec backoff exponentiel sur les erreurs réseau et les 5xx
//!   (les erreurs de parsing ne sont jamais retentées : elles sont déterministes).
//! - `api_key` optionnelle : aucun en-tête d'autorisation fantôme.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::{DEFAULT_LLM_BASE_URL, DEFAULT_LLM_MODEL};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
    /// Nouvelles tentatives sur erreur transport/5xx (0 = aucune).
    pub retry_attempts: u32,
    /// Délai de base du backoff exponentiel (ms).
    pub retry_backoff_ms: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "ollama".into(),
            model: DEFAULT_LLM_MODEL.into(),
            base_url: DEFAULT_LLM_BASE_URL.into(),
            api_key: None,
            timeout_secs: 300,
            retry_attempts: 3,
            retry_backoff_ms: 500,
            max_tokens: 2048,
            temperature: 0.3,
        }
    }
}

pub struct LlmClient {
    client: Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        Self { client, config }
    }

    pub fn is_available(&self) -> bool {
        if self.config.provider == "ollama" {
            self.client
                .get(format!("{}/api/tags", self.config.base_url))
                .send()
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        } else {
            true
        }
    }

    /// Génère du texte. Retente les erreurs transport/5xx avec backoff.
    pub fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String, String> {
        if self.config.provider == "ollama" {
            let body = self.ollama_body(prompt, system);
            let data =
                self.post_with_retry(&format!("{}/api/generate", self.config.base_url), &body)?;
            let text = data["response"].as_str().unwrap_or("");
            let thinking = data["thinking"].as_str().unwrap_or("");
            Ok(if text.is_empty() {
                thinking.to_string()
            } else {
                text.to_string()
            })
        } else {
            let body = self.openai_body(prompt, system);
            let data =
                self.post_with_retry(&format!("{}/chat/completions", self.config.base_url), &body)?;
            Ok(data["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string())
        }
    }

    pub fn generate_json(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let raw = self.generate(prompt, system)?;
        serde_json::from_str(&raw).or_else(|_| {
            // Try to extract JSON from markdown
            if let Some(start) = raw.find('{') {
                if let Some(end) = raw.rfind('}') {
                    serde_json::from_str(&raw[start..=end])
                        .map_err(|e| format!("JSON parse: {}", e))
                } else {
                    Err("No closing brace".into())
                }
            } else {
                Err("No JSON found".into())
            }
        })
    }

    fn ollama_body(&self, prompt: &str, system: Option<&str>) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false,
            "options": {
                "temperature": self.config.temperature,
                "num_predict": self.config.max_tokens,
            }
        });
        if let Some(sys) = system {
            body["system"] = serde_json::Value::String(sys.to_string());
        }
        body
    }

    fn openai_body(&self, prompt: &str, system: Option<&str>) -> serde_json::Value {
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        serde_json::json!({
            "model": self.config.model,
            "messages": messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
        })
    }

    /// POST avec retry exponentiel sur erreurs transport et statuts 5xx.
    fn post_with_retry(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let attempts = self.config.retry_attempts.saturating_add(1);
        let mut last_err = String::new();

        for attempt in 0..attempts {
            if attempt > 0 {
                let backoff = self.backoff_delay(attempt);
                log::warn!(
                    "Nouvelle tentative {}/{} vers {} dans {} ms (cause: {})",
                    attempt,
                    self.config.retry_attempts,
                    url,
                    backoff.as_millis(),
                    last_err
                );
                std::thread::sleep(backoff);
            }

            let mut request = self.client.post(url).json(body);
            if let Some(ref key) = self.config.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }

            match request.send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() {
                        last_err = format!("erreur serveur ({})", status);
                        continue;
                    }
                    if status.is_client_error() || !status.is_success() {
                        // 4xx et autres : non retenté (problème de requête).
                        return Err(format!("requête rejetée ({}) par {}", status, url));
                    }
                    return match response.text() {
                        Ok(text) => serde_json::from_str(&text).map_err(|e| {
                            let preview: String = text.chars().take(200).collect();
                            format!("Parse error: {} (corps reçu: {:?})", e, preview)
                        }),
                        Err(e) => Err(format!("Lecture de la réponse échouée: {}", e)),
                    };
                }
                Err(e) => {
                    last_err = e.to_string();
                }
            }
        }

        Err(format!(
            "Échec après {} tentative(s) vers {}: {}",
            attempts, url, last_err
        ))
    }

    fn backoff_delay(&self, attempt: u32) -> Duration {
        let factor = 2u64.saturating_pow(attempt.min(8));
        let ms = self
            .config
            .retry_backoff_ms
            .saturating_mul(factor)
            .min(30_000);
        Duration::from_millis(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// Petit serveur HTTP mock mono-usage pour tester le client en réel.
    struct MockServer {
        addr: String,
        handle: Option<std::thread::JoinHandle<()>>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockServer {
        /// `responses` : une réponse HTTP brute (statut, corps) par requête acceptée.
        /// Collecte les requêtes reçues pour assertions.
        fn spawn(
            responses: Vec<(u16, &'static str)>,
        ) -> (Self, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::{Arc, Mutex};

            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
            let addr = listener.local_addr().unwrap().to_string();
            let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let req_clone = requests.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let stop_clone = stop.clone();

            let handle = std::thread::spawn(move || {
                listener.set_nonblocking(true).ok();
                for (status, body) in responses {
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    // Accept non bloquant avec attente active courte.
                    let mut stream = loop {
                        if stop_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        match listener.accept() {
                            Ok((s, _)) => break s,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(5));
                            }
                            Err(_) => return,
                        }
                    };
                    if let Some(req) = read_http_request(&mut stream) {
                        req_clone.lock().unwrap().push(req);
                    }
                    let http_status = match status {
                        200 => "200 OK",
                        500 => "500 Internal Server Error",
                        401 => "401 Unauthorized",
                        _ => "500 Internal Server Error",
                    };
                    let payload = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        http_status,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(payload.as_bytes());
                    let _ = stream.flush();
                    // Fermeture propre : shutdown(Write) envoie le FIN après les
                    // données ; un shutdown(Both) immédiat peut générer un RST
                    // qui détruit des octets non encore lus par le client.
                    let _ = stream.shutdown(std::net::Shutdown::Write);
                    drop(stream);
                }
            });

            (
                MockServer {
                    addr,
                    handle: Some(handle),
                    stop,
                },
                requests,
            )
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            use std::sync::atomic::Ordering;
            self.stop.store(true, Ordering::Relaxed);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    /// Lit une requête HTTP (request line + headers + body éventuel).
    fn read_http_request(stream: &mut TcpStream) -> Option<String> {
        let mut reader = BufReader::new(stream.try_clone().ok()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).ok()?;
        let mut raw = request_line.clone();
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).ok()?;
            if n == 0 || line.trim().is_empty() {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = v.trim().parse().ok()?;
            }
            raw.push_str(&line);
        }
        if content_length > 0 {
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).ok()?;
            raw.push_str(&String::from_utf8_lossy(&body));
        }
        Some(raw)
    }

    fn fast_config(base_url: String, retries: u32) -> LlmConfig {
        LlmConfig {
            provider: "openai".into(),
            base_url,
            api_key: None,
            timeout_secs: 5,
            retry_attempts: retries,
            retry_backoff_ms: 1,
            ..LlmConfig::default()
        }
    }

    #[test]
    fn openai_request_has_no_auth_header_without_api_key() {
        let body = r#"{"choices":[{"message":{"content":"pong"}}]}"#;
        let (server, requests) = MockServer::spawn(vec![(200, body)]);
        let client = LlmClient::new(fast_config(format!("http://{}", server.addr), 0));

        let out = client.generate("ping", None).expect("should succeed");
        assert_eq!(out, "pong");

        let reqs = requests.lock().unwrap();
        assert_eq!(reqs.len(), 1);
        assert!(
            reqs[0].contains("POST /chat/completions"),
            "got: {}",
            reqs[0]
        );
        assert!(
            !reqs[0].to_lowercase().contains("authorization"),
            "aucun header fantôme"
        );
    }

    #[test]
    fn openai_request_sends_bearer_token_when_configured() {
        let body = r#"{"choices":[{"message":{"content":"ok"}}]}"#;
        let (server, requests) = MockServer::spawn(vec![(200, body)]);
        let config = LlmConfig {
            api_key: Some("sk-secret".into()),
            ..fast_config(format!("http://{}", server.addr), 0)
        };
        let client = LlmClient::new(config);

        client.generate("hi", Some("sys")).unwrap();

        let reqs = requests.lock().unwrap();
        assert!(
            reqs[0].contains("Bearer sk-secret"),
            "header Authorization attendu, got: {}",
            reqs[0]
        );
        assert!(
            reqs[0].contains("system"),
            "le message système doit être transmis"
        );
    }

    #[test]
    fn retries_on_5xx_then_succeeds() {
        let ok = r#"{"choices":[{"message":{"content":"after-retry"}}]}"#;
        let (server, requests) = MockServer::spawn(vec![(500, "{}"), (500, "{}"), (200, ok)]);
        let client = LlmClient::new(fast_config(format!("http://{}", server.addr), 2));

        let out = client
            .generate("q", None)
            .expect("doit réussir au 3e essai");
        assert_eq!(out, "after-retry");
        assert_eq!(requests.lock().unwrap().len(), 3);
    }

    #[test]
    fn exhausts_retries_and_returns_clear_error() {
        let (server, requests) = MockServer::spawn(vec![(500, "{}"); 4]);
        let client = LlmClient::new(fast_config(format!("http://{}", server.addr), 2));

        let err = client.generate("q", None).expect_err("doit échouer");
        assert!(err.contains("Échec après 3 tentative"), "err: {err}");
        assert_eq!(requests.lock().unwrap().len(), 3, "1 initiale + 2 retries");
    }

    #[test]
    fn client_error_is_not_retried() {
        let (server, requests) = MockServer::spawn(vec![(401, r#"{"error":"bad key"}"#)]);
        let client = LlmClient::new(fast_config(format!("http://{}", server.addr), 3));

        let err = client.generate("q", None).expect_err("401 doit échouer");
        assert!(err.contains("401"), "err: {err}");
        assert_eq!(requests.lock().unwrap().len(), 1, "pas de retry sur 4xx");
    }

    #[test]
    fn ollama_provider_prefers_response_over_thinking() {
        let (server, _requests) = MockServer::spawn(vec![(
            200,
            r#"{"response":"réponse finale","thinking":"chaîne de pensée"}"#,
        )]);
        let config = LlmConfig {
            provider: "ollama".into(),
            ..fast_config(format!("http://{}", server.addr), 0)
        };
        let client = LlmClient::new(config);
        assert_eq!(client.generate("q", None).unwrap(), "réponse finale");
    }

    #[test]
    fn generate_json_extracts_from_markdown_fence() {
        let (server, _) = MockServer::spawn(vec![(
            200,
            r##"{"choices":[{"message":{"content":"Voici le résultat:\n```json\n{\"score\": 7}\n```"}}]}"##,
        )]);
        let client = LlmClient::new(fast_config(format!("http://{}", server.addr), 0));
        let v = client.generate_json("q", None).unwrap();
        assert_eq!(v["score"], 7);
    }
}
