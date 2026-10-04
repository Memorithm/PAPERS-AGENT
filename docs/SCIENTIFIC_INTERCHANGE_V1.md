# Scientific Interchange v1 — PAPERS → CCOS Research Lab → RSI

## Purpose

PAPERS is the scientific intake and hypothesis-generation layer. It is not the authority that decides whether a paper's claim is true or whether generated code is an improvement.

The v1 boundary is intentionally functional and versioned:

```text
paper / PDF / arXiv
        |
        v
      PAPERS
  extract + analyse
        |
        v
memorithm.science/bundle-v1
        |
        +----------------------+
        |                      |
        v                      v
CCOS Research Lab            RSI / rsi-scholar
experiment + audit           candidate generation
        |                      |
        +----------+-----------+
                   v
          empirical evaluator
                   |
                   v
       ExperimentResult / evidence
```

PAPERS produces claims and provenance. A typed planner may produce experiment proposals. CCOS Research Lab and RSI execute real experiments/builds/tests/benchmarks. Only empirical results may move a claim to `reproduced`, `partially_reproduced` or `contradicted`.

## Trust classes

`EvidenceOrigin` makes the provenance boundary explicit:

- `source_span`: text directly located in the source document;
- `analysis_field`: structured output created by the PAPERS analysis pipeline;
- `model_inference`: a statement inferred by a model;
- `experiment`: an observation produced by an executed experiment.

These classes are not interchangeable. A model summary is not source evidence. A paper claim is not an empirical result from our environment.

## Schemas

The Rust definitions live in `papers_core/src/scientific_contract.rs`.

### `memorithm.science/bundle-v1`

Contains:

- paper identity;
- `ScientificClaim[]`;
- `ExperimentProposal[]` (empty unless a typed planner has enough information to construct one without guessing);
- provenance for the exact extracted content and analysis report.

### `memorithm.science/claim-v1`

A claim contains the statement, kind, state, evidence locators, assumptions, method/algorithm fields, metrics/effects when known, limitations, falsification criteria, optional confidence and provenance.

PAPERS must leave a field empty/`null` when the information is not defensibly available. It must not fill missing scientific information with defaults.

### `memorithm.science/experiment-proposal-v1`

An experiment proposal requires an explicit hypothesis, target component, intervention, baseline and at least one repetition. It can carry metrics, workload, acceptance/rejection criteria, resource limits and safety constraints.

The v1 `AnalysisReport → ScientificBundle` conversion intentionally creates **no automatic proposals**, because the current untyped analysis does not always contain a defensible baseline, workload and acceptance criterion.

### `memorithm.science/experiment-result-v1`

A result links back to a proposal and records status, finite metric observations, uncertainty/sample counts when available, evidence/artifacts and provenance.

## Determinism and provenance

The converter records:

- SHA-256 of the exact extracted full text, abstract or metadata available to PAPERS;
- the corresponding content scope (`full_text`, `abstract`, `metadata`);
- SHA-256 of the serialized `AnalysisReport`;
- PAPERS generator name/version;
- the original analysis timestamp, so re-exporting the same report is replay-friendly;
- optional model provider/model/config SHA-256 when the caller actually knows them.

Claim identifiers are deterministic domain-separated hashes of paper id, ordinal and statement.

## CLI

Convert an existing analysis without linking PAPERS into RSI or CCOS:

```bash
cd papers_core
cargo run --bin papers-contract -- \
  --input ./output/analysis.json \
  --output ./output/scientific_bundle.json
```

For model-backed analysis, record the model explicitly:

```bash
cargo run --bin papers-contract -- \
  --input ./output/analysis.json \
  --output ./output/scientific_bundle.json \
  --provider ollama \
  --model '<exact-model-name>'
```

`--provider` and `--model` are an all-or-nothing pair. PAPERS does not guess them from an old JSON file that did not record them.

## Empirical evaluation boundary

`WasmExecutor::execute_trusted_wasm` still executes genuine WASM with Wasmtime fuel and epoch deadlines.

`WasmExecutor::execute_rust_source` validates the source-size budget and requires an explicit PAPERS entrypoint, then **fails closed before launching local rustc**. Generated or third-party Rust must cross the external SciRust-Hub/RemoteOps OS-isolated runtime boundary. The crate retains a deliberately named trusted-only local compiler helper for maintainer diagnostics; it is not the generated-code execution path.

The local Wasmtime path applies a module byte limit plus fuel/epoch deadlines and `StoreLimits` for memories, tables and instance counts. Those controls bound the in-process diagnostic path; they are not presented as an OS hostile-code sandbox or as a bound on all JIT/compiler memory. Untrusted JIT execution therefore still belongs behind the external qualified runtime.

Compilation success and Wasmtime runtime success are **diagnostic evidence only**. They record runtime status, fuel consumed and duration, but produce zero empirical fitness. The evolution loop may rank a candidate only after an explicit task evaluator establishes `task_oracle` authority. Structural checks likewise remain diagnostic.

For repository improvements the preferred empirical path is RSI/CCOS Research Lab:

```text
PAPERS claim/method
      ↓
experiment or directive goal
      ↓
GuardedDgm / isolated workspace
      ↓
cargo build + test + benchmark
      ↓
accepted/rejected evidence
```

## SciRust boundary

PAPERS core no longer assumes `/tmp/scirust/...` at build time.

The local core keeps deterministic baselines for fallback embedding and numerical operations. NAS fails closed instead of fabricating fitness. Advanced symbolic search, NAS and scientific kernels are expected to cross a runtime/versioned boundary toward SciRust/CCOS Research Lab, where their outputs can be benchmarked and audited.

This deliberately preserves the architecture already used by RSI's PAPERS addon: heavy research engines remain external capabilities, while the trusted evaluator stays authoritative.

## Next contract work

The next compatible additions are:

1. source-span locators with page/section offsets from PDF ingestion;
2. typed extraction of datasets, baselines, metrics and limitations into claims;
3. a CCOS Research Lab importer that records bundle/claim hashes in its hash-chained event log;
4. an RSI adapter that consumes claim/method IDs rather than parsing free-form `analysis.json` fields;
5. `ExperimentResult` feedback from CCOS/RSI to the scientific knowledge graph.

Breaking changes require a new schema tag; v1 consumers must reject unsupported tags rather than silently reinterpret them.
