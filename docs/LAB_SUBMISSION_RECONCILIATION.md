# Submission outcomes and read-only reconciliation

`LabClient::submit_experiment` sends one POST only. `with_retry` applies to
GET status/result reads, not submissions. HTTP redirects are disabled: a 307
must not secretly replay the POST elsewhere. PAPERS does not pretend that an
idempotency header alone implements server-side durable deduplication.

A transport failure, non-2xx response, lost/non-JSON acknowledgement, empty
ID or missing/invalid state returns an error prefixed `unknown_outcome:`.
The lab may already have accepted the experiment. No retry is performed and
no default queued state is fabricated. A valid successful acknowledgement
continues to return the existing `Submission` type. The String error API is
preserved for existing consumers.

Do not rerun `papers-contract --lab-url` to resolve an unknown outcome. Keep
the exported bundle and diagnostics. If a lab experiment ID is known from
an acknowledgement or the lab's authoritative logs, use:

```sh
cd papers_core
cargo run --bin papers-lab -- reconcile \
  --lab-url http://127.0.0.1:8080 --experiment-id '<lab-id>'
```

An optional bearer key is read from `PAPERS_LAB_API_KEY`, not a command-line
argument. The command only calls the existing GET status API and prints its
state; it never submits, promotes or fabricates empirical results.

If no ID was received, this minimal lab protocol has no lookup-by-request
API. Reconciliation must use the lab's logs; the client cannot prove that
absence of an acknowledgement means absence of an experiment. That outcome
remains unknown until authoritative evidence exists. Durable server-side
idempotency would require a separately qualified client/server contract,
not an unverified request key. This change chooses the audit's no-retry
alternative rather than inventing that contract.

## Regression validation

The `ccos` unit tests preserve successful submissions and GET retries and
add bounded loopback probes proving exactly one POST after a 5xx, an accepted
request whose acknowledgement is lost, a malformed acknowledgement, and a
307 response. `papers-lab` argument tests reject missing identifiers and
any submit command. Full exact-head CI and integrated-branch validation
remain required before closing PAPERS-03. Mock protocol tests do not qualify
the real Research Lab or an empirical execution backend.
