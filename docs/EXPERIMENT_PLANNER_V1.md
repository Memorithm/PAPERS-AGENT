# Explicit experiment planner v1

`papers-experiment` creates `memorithm.science/experiment-proposal-v1` from a previously validated scientific bundle.

It is deliberately **not** an autonomous planner: every field that can determine experimental truth is supplied explicitly by the operator/agent. The command never executes the intervention.

## Required inputs

- one or more `--claim <claim-id>` values that must exist in the bundle;
- `--hypothesis`;
- `--target`;
- `--intervention`;
- `--baseline`;
- at least one `--metric`;
- at least one `--accept` criterion.

Optional fields include rejection criteria, safety constraints, expected direction/effect, workload, seed, repetitions and resource limits.

Example:

```bash
papers-experiment \
  --bundle output/scientific_bundle.json \
  --claim method-fixture-1 \
  --hypothesis 'TiledMethod lowers median latency without changing outputs' \
  --target src/kernel.rs \
  --intervention 'candidate implementation using TiledMethod' \
  --baseline 'current main implementation' \
  --metric latency_ns \
  --accept 'median latency improves by at least 5% and all tests pass' \
  --reject 'any correctness regression' \
  --safety offline \
  --repetitions 20 \
  --timeout-seconds 60 \
  --max-memory-bytes 4294967296 \
  --max-output-bytes 8388608 \
  --output output/experiment_proposal.json
```

## Determinism

The proposal id is a SHA-256-derived stable id over the paper id and all experiment-defining inputs. Identical explicit inputs therefore produce the same proposal id.

The bundle provenance is copied into the proposal. No missing baseline, metric, criterion or resource limit is inferred from prose.

## Execution boundary

The output is data. CCOS Research Lab can validate and attest it with `import_experiment_proposal`, but import does not schedule or execute it. A separate typed sandbox/evaluator must map the proposal to an allowed workload. RSI/GuardedDgm remains the code-promotion authority.
