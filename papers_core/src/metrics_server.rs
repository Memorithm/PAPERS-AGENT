//! Métriques Prometheus pour PAPERS.
//!
//! Installe un recorder global (`metrics` crate) et expose les compteurs via
//! un petit serveur HTTP `/metrics` sans dépendance supplémentaire :
//!
//! ```text
//! papers serve-metrics --port 9091
//! curl http://127.0.0.1:9091/metrics
//! ```
//!
//! Compteurs exposés :
//! - `papers_extractions_total` : documents extraits
//! - `papers_analyses_total` : analyses réalisées
//! - `papers_evolutions_total` : boucles d'évolution lancées
//! - `papers_llm_calls_total` / `papers_llm_failures_total` : trafic LLM
//! - `papers_watch_papers_found_total` : nouveaux papiers détectés par watch

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Recorder global installé une seule fois par processus.
/// `None` si l'installation a échoué (ex. recorder déjà posé).
static RENDER_HANDLE: OnceLock<Option<PrometheusHandle>> = OnceLock::new();

/// Initialise le recorder global de métriques (idempotent).
pub fn init_metrics() {
    RENDER_HANDLE.get_or_init(|| PrometheusBuilder::new().install_recorder().ok());
}

/// Rendu texte exposition format Prometheus (vide si non initialisé).
pub fn render_metrics() -> String {
    match RENDER_HANDLE.get().and_then(|opt| opt.as_ref()) {
        Some(handle) => handle.render(),
        None => String::new(),
    }
}

pub fn record_extraction() {
    metrics::counter!("papers_extractions_total").increment(1);
}

pub fn record_analysis() {
    metrics::counter!("papers_analyses_total").increment(1);
}

pub fn record_evolution() {
    metrics::counter!("papers_evolutions_total").increment(1);
}

pub fn record_llm_call() {
    metrics::counter!("papers_llm_calls_total").increment(1);
}

pub fn record_llm_failure() {
    metrics::counter!("papers_llm_failures_total").increment(1);
}

pub fn record_watch_paper() {
    metrics::counter!("papers_watch_papers_found_total").increment(1);
}

/// Démarre un serveur HTTP minimal exposant `/metrics` sur `addr`.
///
/// Retourne immédiatement après avoir démarré le thread serveur ; le thread
/// vit jusqu'à la fin du processus (comportement attendu d'un exporter).
pub fn serve_metrics(addr: &str) -> std::io::Result<std::thread::JoinHandle<()>> {
    init_metrics();
    let listener = TcpListener::bind(addr)?;
    log::info!(
        "Métriques Prometheus exposées sur http://{}/metrics",
        listener.local_addr()?
    );

    std::thread::Builder::new()
        .name("metrics-server".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                handle_connection(&mut { stream });
            }
        })
}

fn handle_connection(stream: &mut TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    match path {
        "/metrics" | "/metrics/" => {
            let body = render_metrics();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
        _ => {
            let body = "404 not found\n";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    }
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    #[test]
    fn counters_increment_and_render_after_init() {
        init_metrics(); // idempotent
        record_extraction();
        record_extraction();
        record_analysis();
        record_evolution();
        record_llm_call();
        record_llm_failure();

        let rendered = render_metrics();
        assert!(rendered.contains("papers_extractions_total"), "{rendered}");
        assert!(rendered.contains("papers_analyses_total"));
        assert!(rendered.contains("papers_evolution"));
        assert!(
            rendered.contains("papers_extractions_total 2"),
            "deux extractions attendues:\n{rendered}"
        );
    }

    /// Version testable : démarre sur port éphémère et retourne l'adresse réelle.
    fn serve_metrics_for_test(
    ) -> std::io::Result<(std::net::SocketAddr, std::thread::JoinHandle<()>)> {
        init_metrics();
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let handle = std::thread::Builder::new()
            .name("metrics-server-test".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut s = stream;
                    handle_connection(&mut s);
                }
            })?;
        Ok((addr, handle))
    }

    #[test]
    fn metrics_serves_on_ephemeral_port() {
        // Le recorder doit être installé avant tout enregistrement : les
        // incréments antérieurs partent dans le recorder no-op global.
        init_metrics();
        record_watch_paper();
        let (addr, handle) = serve_metrics_for_test().unwrap();

        let mut stream = TcpStream::connect(addr).expect("connexion");
        stream
            .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(
            response.contains("papers_watch_papers_found_total"),
            "{response}"
        );

        // Route inconnue → 404.
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(b"GET /unknown HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        reader.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 404"));

        // Le thread serveur tourne jusqu'à la fin du process : on le détache
        // (join bloquerait indéfiniment sur la boucle d'accept).
        drop(handle);
    }
}
