//! Client pour le CCOS Research Lab / RSI.
//!
//! PAPERS produit des hypothèses scientifiques versionnées
//! (`memorithm.science/bundle-v1`) ; l'exécution empirique appartient au
//! Research Lab. Ce client couvre le protocole minimal :
//!
//! - [`LabClient::submit_experiment`] : POST `/api/v1/experiments`
//! - [`LabClient::experiment_status`] : GET `/api/v1/experiments/{id}`
//! - [`LabClient::experiment_result`] : GET `/api/v1/experiments/{id}/result`
//! - [`LabClient::wait_for_completion`] : polling jusqu'à un état terminal
//!
//! Aucun score n'est inventé localement : sans lab joignable, les méthodes
//! retournent une erreur explicite.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// États de cycle de vie d'une expérience côté lab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentState {
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    /// État inconnu du client (forward-compatibilité).
    #[serde(untagged)]
    Unknown(String),
}

impl ExperimentState {
    /// Vrai si l'état est terminal (plus besoin de poller).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// Vrai si l'expérience s'est terminée avec succès.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Accusé de soumission renvoyé par le lab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: String,
    pub state: ExperimentState,
}

/// Statut courant d'une expérience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentStatus {
    pub id: String,
    pub state: ExperimentState,
    /// Charge utile complète renvoyée par le lab (diagnostic).
    #[serde(default)]
    pub raw: serde_json::Value,
}

/// Client HTTP du CCOS Research Lab.
pub struct LabClient {
    http: reqwest::blocking::Client,
    base_url: String,
    api_key: Option<String>,
    retry_attempts: u32,
    retry_backoff_ms: u64,
}

impl LabClient {
    /// Crée un client pointant sur `base_url` (ex. `http://127.0.0.1:8080`).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
            base_url: base_url.into(),
            api_key: None,
            retry_attempts: 2,
            retry_backoff_ms: 250,
        }
    }

    /// Définit une clé API (en-tête `Authorization: Bearer …`).
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Ajuste la politique de retry transport/5xx.
    pub fn with_retry(mut self, attempts: u32, backoff_ms: u64) -> Self {
        self.retry_attempts = attempts;
        self.retry_backoff_ms = backoff_ms;
        self
    }

    /// Soumet un bundle scientifique ; retourne l'accusé avec son identifiant.
    pub fn submit_experiment(&self, bundle: &serde_json::Value) -> Result<Submission, String> {
        let data = self.send(
            reqwest::Method::POST,
            &format!("{}/api/v1/experiments", self.base_url),
            Some(bundle),
        )?;
        let id = data["id"]
            .as_str()
            .ok_or("réponse de soumission sans champ 'id'")?
            .to_string();
        let state =
            serde_json::from_value(data["state"].clone()).unwrap_or(ExperimentState::Queued);
        Ok(Submission { id, state })
    }

    /// Interroge le statut d'une expérience.
    pub fn experiment_status(&self, id: &str) -> Result<ExperimentStatus, String> {
        let data = self.send(
            reqwest::Method::GET,
            &format!("{}/api/v1/experiments/{id}", self.base_url),
            None,
        )?;
        let status: ExperimentStatus =
            serde_json::from_value(data.clone()).map_err(|e| format!("statut illisible: {e}"))?;
        Ok(status)
    }

    /// Récupère le résultat final d'une expérience terminée.
    pub fn experiment_result(&self, id: &str) -> Result<serde_json::Value, String> {
        self.send(
            reqwest::Method::GET,
            &format!("{}/api/v1/experiments/{id}/result", self.base_url),
            None,
        )
    }

    /// Poll jusqu'à un état terminal ou expiration du délai.
    pub fn wait_for_completion(
        &self,
        id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<ExperimentStatus, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = self.experiment_status(id)?;
            if status.state.is_terminal() {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "délai dépassé en attendant la fin de l'expérience {id} (état: {:?})",
                    status.state
                ));
            }
            std::thread::sleep(poll_interval);
        }
    }

    fn send(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let attempts = self.retry_attempts.saturating_add(1);
        let mut last_err = String::new();

        for attempt in 0..attempts {
            if attempt > 0 {
                let backoff = self
                    .retry_backoff_ms
                    .saturating_mul(2u64.saturating_pow(attempt.min(8)))
                    .min(10_000);
                log::warn!(
                    "Lab: nouvelle tentative {}/{} dans {backoff} ms vers {} ({last_err})",
                    attempt,
                    self.retry_attempts,
                    url
                );
                std::thread::sleep(Duration::from_millis(backoff));
            }

            let mut request = self.http.request(method.clone(), url);
            if let Some(ref key) = self.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }
            if let Some(b) = body {
                request = request.json(b);
            }

            match request.send() {
                Ok(response) => {
                    let status = response.status();
                    if status.is_server_error() {
                        last_err = format!("erreur serveur ({status})");
                        continue;
                    }
                    if !status.is_success() {
                        return Err(format!("requête rejetée ({status}) par {url}"));
                    }
                    return response
                        .json()
                        .map_err(|e| format!("réponse non-JSON de {url}: {e}"));
                }
                Err(e) => last_err = e.to_string(),
            }
        }

        Err(format!(
            "échec après {attempts} tentative(s) vers {url}: {last_err}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// Mock HTTP : lit entièrement chaque requête (évite tout RST), répond selon
    /// une file de réponses (statut, corps).
    fn spawn_mock(responses: Vec<(u16, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut content_length = 0usize;
                let mut first_line = String::new();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if first_line.is_empty() {
                        first_line = line.trim().to_string();
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                if content_length > 0 {
                    let mut sink = vec![0u8; content_length];
                    let _ = reader.read_exact(&mut sink);
                }
                let _ = &first_line;
                let code = match status {
                    200 => "200 OK",
                    201 => "201 Created",
                    _ => "500 Internal Server Error",
                };
                let payload = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    code,
                    body.len(),
                    body
                );
                stream.write_all(payload.as_bytes()).unwrap();
                stream.flush().unwrap();
                stream.shutdown(std::net::Shutdown::Write).ok();
            }
        });
        addr
    }

    fn client_at(addr: &str) -> LabClient {
        LabClient::new(format!("http://{addr}")).with_retry(0, 1)
    }

    #[test]
    fn submit_returns_id_and_queued_state() {
        let addr = spawn_mock(vec![(201, r#"{"id":"exp-123","state":"queued"}"#)]);
        let client = client_at(&addr);
        let sub = client
            .submit_experiment(&serde_json::json!({"schema": "memorithm.science/bundle-v1"}))
            .expect("soumission");
        assert_eq!(sub.id, "exp-123");
        assert_eq!(sub.state, ExperimentState::Queued);
        assert!(!sub.state.is_terminal());
    }

    #[test]
    fn status_maps_terminal_states() {
        let addr = spawn_mock(vec![(200, r#"{"id":"x","state":"completed"}"#)]);
        let st = client_at(&addr).experiment_status("x").unwrap();
        assert_eq!(st.state, ExperimentState::Completed);
        assert!(st.state.is_terminal());
        assert!(st.state.is_success());

        let addr2 = spawn_mock(vec![(200, r#"{"id":"y","state":"failed"}"#)]);
        let st2 = client_at(&addr2).experiment_status("y").unwrap();
        assert!(st2.state.is_terminal() && !st2.state.is_success());
    }

    #[test]
    fn unknown_state_is_forward_compatible() {
        let addr = spawn_mock(vec![(200, r#"{"id":"z","state":"quantum-superposed"}"#)]);
        let st = client_at(&addr).experiment_status("z").unwrap();
        assert_eq!(
            st.state,
            ExperimentState::Unknown("quantum-superposed".into())
        );
        assert!(
            !st.state.is_terminal(),
            "état inconnu ≠ terminal → on continue à poller"
        );
    }

    #[test]
    fn wait_for_completion_polls_until_terminal() {
        let responses = vec![
            (200, r#"{"id":"w","state":"running"}"#),
            (200, r#"{"id":"w","state":"running"}"#),
            (200, r#"{"id":"w","state":"completed"}"#),
        ];
        let addr = spawn_mock(responses);
        let client = client_at(&addr).with_retry(0, 1);
        let st = client
            .wait_for_completion("w", Duration::from_secs(5), Duration::from_millis(5))
            .unwrap();
        assert_eq!(st.state, ExperimentState::Completed);
    }

    #[test]
    fn wait_for_completion_times_out_with_clear_error() {
        // Assez de réponses non terminales pour couvrir toute la fenêtre de
        // polling : l'erreur doit venir du délai, pas d'un échec transport.
        let responses: Vec<_> =
            std::iter::repeat_n((200u16, r#"{"id":"t","state":"running"}"#), 50).collect();
        let addr = spawn_mock(responses);
        let client = client_at(&addr);
        let err = client
            .wait_for_completion("t", Duration::from_millis(300), Duration::from_millis(20))
            .expect_err("timeout attendu");
        assert!(err.contains("délai dépassé"), "err: {err}");
    }

    #[test]
    fn server_errors_are_retried_then_succeed() {
        let responses = vec![
            (500, r#"{}"#),
            (500, r#"{}"#),
            (200, r#"{"id":"r","state":"queued"}"#),
        ];
        let addr = spawn_mock(responses);
        let client = client_at(&addr).with_retry(2, 1);
        let sub = client.submit_experiment(&serde_json::json!({})).unwrap();
        assert_eq!(sub.id, "r");
    }

    #[test]
    fn result_endpoint_roundtrips_payload() {
        let addr = spawn_mock(vec![(200, r#"{"fitness":{"best_score":0.87},"runs":3}"#)]);
        let res = client_at(&addr).experiment_result("q").unwrap();
        assert_eq!(res["fitness"]["best_score"], 0.87);
        assert_eq!(res["runs"], 3);
    }
}
