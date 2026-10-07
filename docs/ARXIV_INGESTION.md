# arXiv metadata ingestion

Both `ArxivExtractor` and `search_arxiv_latest` use the policy in
`papers_core/src/arxiv_http.rs`: the fixed HTTPS Atom endpoint
`https://export.arxiv.org/api/query`, HTTPS-only requests, no redirects,
a 10-second connect timeout and a 30-second total request/body deadline.
Client construction errors are returned, never replaced by an unconfigured
client. Only successful 2xx responses may reach the existing metadata parser.

The body budget is 2 MiB of delivered bytes. An oversized Content-Length is
rejected before reading. Streaming reads consume at most the budget plus one
sentinel byte, so omitted lengths and chunked transfer cannot bypass the cap.
Oversize, invalid UTF-8, failed status and timeout responses produce errors,
not truncated metadata or partial success. Large searches can therefore fail
explicitly and should be divided by the caller; there is no silent downgrade.

## Provenance boundary

These calls obtain metadata and abstracts from arXiv, not paper PDFs or
independent empirical evidence. The returned source remains `arXiv`, with
the paper ID and available source URL. URLs present in a feed are metadata;
the extractor does not follow them. The existing scientific interchange
records the hash of the extracted content/abstract/metadata and its scope
(see [SCIENTIFIC_INTERCHANGE_V1.md](SCIENTIFIC_INTERCHANGE_V1.md)). It does
not currently retain a hash or timestamp of the raw Atom response: this
transport hardening does not claim raw-feed archival or new empirical authority.

## Regression checks

`cargo test --lib arxiv_http::tests` covers exact/oversized bodies, invalid
UTF-8, huge declared length, chunked oversize, 302 refusal without following,
503 refusal, the production client's plain-HTTP rejection, and a delayed
loopback response hitting the total timeout. The fixture is bounded and
local; it does not contact arXiv. Existing extraction/feed parser tests remain
unchanged. Full CI must pass on the PR head and on the integrated branch
before the audit finding PAPERS-04 can be closed.
