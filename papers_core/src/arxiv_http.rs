//! Shared, fail-closed transport policy for arXiv Atom metadata.

use std::io::Read;
use std::time::Duration;

use reqwest::blocking::{Client, ClientBuilder, Response};

pub(crate) const API_URL: &str = "https://export.arxiv.org/api/query";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

fn configured_builder(timeout: Duration) -> ClientBuilder {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
}

pub(crate) fn client() -> Result<Client, String> {
    configured_builder(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("client HTTPS arXiv: {e}"))
}

pub(crate) fn read_response(response: Response) -> Result<String, String> {
    read_response_with_limit(response, MAX_BODY_BYTES)
}

fn read_response_with_limit(response: Response, limit: usize) -> Result<String, String> {
    // error_for_status alone permits 3xx; redirects must also fail closed.
    if !response.status().is_success() {
        return Err(format!("statut HTTP arXiv refusé: {}", response.status()));
    }
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(format!("réponse arXiv au-delà de {limit} octets"));
    }
    read_bounded(response, limit)
}

fn read_bounded(reader: impl Read, limit: usize) -> Result<String, String> {
    // One sentinel byte detects oversize bodies even without Content-Length.
    // Reserve only the bounded budget, never a server-supplied length.
    let budget = limit.checked_add(1).ok_or("budget arXiv invalide")?;
    let mut bytes = Vec::with_capacity(budget);
    reader
        .take(budget as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("lecture arXiv: {e}"))?;
    if bytes.len() > limit {
        return Err(format!("réponse arXiv au-delà de {limit} octets"));
    }
    String::from_utf8(bytes).map_err(|e| format!("réponse arXiv non UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    // A bounded loopback fixture; never contacts arXiv or production.
    fn fixture(response: Vec<u8>, delay: Duration) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut request = [0; 4096];
                        let _ = stream.read(&mut request);
                        thread::sleep(delay);
                        let _ = stream.write_all(&response);
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "fixture accept timed out");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
        });
        (url, handle)
    }

    fn local_response(wire: &[u8]) -> Response {
        let (url, handle) = fixture(wire.to_vec(), Duration::ZERO);
        // Only this test client permits loopback HTTP; keep all other policy.
        let response = configured_builder(Duration::from_secs(2))
            .https_only(false)
            .no_proxy()
            .build()
            .unwrap()
            .get(url)
            .send()
            .unwrap();
        handle.join().unwrap();
        response
    }

    #[test]
    fn exact_limit_is_accepted_and_sentinel_is_rejected() {
        assert_eq!(read_bounded(&b"12345678"[..], 8).unwrap(), "12345678");
        assert!(read_bounded(&b"123456789"[..], 8).is_err());
        assert!(read_bounded(&b"\xff"[..], 8).is_err());
    }

    #[test]
    fn oversized_declared_and_chunked_bodies_are_rejected() {
        let declared = local_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\nConnection: close\r\n\r\n",
        );
        assert!(read_response_with_limit(declared, 8)
            .unwrap_err()
            .contains("au-delà"));
        let chunked = local_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n");
        assert!(read_response_with_limit(chunked, 8)
            .unwrap_err()
            .contains("au-delà"));
    }

    #[test]
    fn redirects_and_error_statuses_are_not_parsed_or_followed() {
        // Following Location would produce a connection error, not a 302 response.
        let redirect = local_response(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        assert_eq!(redirect.status(), reqwest::StatusCode::FOUND);
        assert!(read_response(redirect).unwrap_err().contains("302"));
        let error = local_response(
            b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        assert!(read_response(error).unwrap_err().contains("503"));
    }

    #[test]
    fn production_client_rejects_plain_http() {
        assert!(client()
            .unwrap()
            .get("http://127.0.0.1:1/")
            .send()
            .unwrap_err()
            .is_builder());
        assert!(API_URL.starts_with("https://"));
    }

    #[test]
    fn delayed_response_obeys_total_timeout() {
        let (url, handle) = fixture(
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            Duration::from_millis(500),
        );
        let result = configured_builder(Duration::from_millis(100))
            .https_only(false)
            .no_proxy()
            .build()
            .unwrap()
            .get(url)
            .send();
        assert!(result.unwrap_err().is_timeout());
        handle.join().unwrap();
    }
}
