# Qualified Hub gVisor execution

PAPERS sends untrusted Rust candidates through the typed SciRust Hub gVisor adapter. The boundary uses Docker with the `runsc` runtime, a digest-pinned image, no network, a read-only root filesystem, dropped capabilities, an executor-owned workspace, and explicit CPU, memory, PID, wall-clock, source, and output bounds.

The consumer implementation is pinned to SciRust Hub commit `cf5233af12499605bd57d9d613b3cddc3bc39d08`. That revision passed Hub CI and the real gVisor qualification run [38517039600](https://github.com/Memorithm/scirust-hub/actions/runs/38517039600), including runtime-entry timeout/cleanup, output cap, positive read-only-root mount evidence, OS per-file ceiling, memory/swap refusal and independently qualified guest PID limits. PAPERS retains both the qualification manifest and the exact receipt whose SHA-256 is `ffb28630643b58d001794117cec6a3b611db04ced7747c583f9571e48b1d6d93`.

The client passes both retained artifacts to the exact Hub binary. Hub refuses to claim qualified isolation unless the compile-time source commit matches the manifest, the receipt hash matches, and the receipt proves a successful real runsc probe. PAPERS then validates the versioned execution receipt and its immutable evidence identifier before accepting the result.

The dedicated GitHub Actions workflow rebuilds that exact Hub commit, installs the checksum-pinned gVisor release, and runs the ignored live integration test. Ordinary unit tests use a bounded fake adapter and never claim real isolation.

Compilation and execution now use two independent requests to that unchanged v1 owner contract. `HubGvisorConfig` requires an explicit `compile_timeout` and retains `timeout` for execution; operators must supply both budgets when constructing the Rust config. Each phase receives a distinct task ID and its full own budget at dispatch (the total may be the sum of both budgets plus adapter overhead, not the old combined deadline). Compilation failure or timeout never dispatches execution. Commands are argv vectors, not interpolated source: `rustc main.rs -o candidate`, then `./candidate`. Both phases retain the same qualified owner admission and CPU/RAM/PID/network/root controls.

Before execution, the produced native artifact must be a nonempty regular, nonsymlink file no larger than 16 MiB; its read is also capped and its SHA-256 is retained. The pinned owner additionally enforces a 16 MiB OS per-file write ceiling. Neither limit is an **OS aggregate disk quota** for many files. Selected phase provenance includes source/artifact/request/receipt digests, task IDs, independent budgets and outcomes; Engineer and evolution nodes retain these facts without upgrading their scientific authority.

The outer adapter uses the same pinned `hub_executor::execution_guard` Rust API
as the owner, rather than a second raw-child/drain implementation. Client process
groups, nonblocking reap confirmation and EOF drainage are bounded and fail closed.
Both phase budgets must be 1 ms..1 hour; outer supervision includes the existing
15-second control margin. A failed adapter/admission/receipt phase retains its
task directory for quarantine and reconciliation, including verified earlier
compile facts when available. No ambiguous workspace is automatically deleted or
replayed. Hub now journals controller attempts durably, separately from PAPERS' retained
domain refusal. A journaled process cleanup is not task success, and an unknown
container remains quarantined until an exact-ID absence observation permits
reclamation. Retaining the directory alone never proves daemon cleanup.

The live gate exercises successful compilation/execution via Engineer, actual compiler-error refusal with no execution phase, runtime timeout after the candidate flushes an entry marker, and actual output-limit refusal via Engineer with the refused workspace retained. Post-compilation admission or execution refusals retain the verified compilation facts through `HubGvisorRefusal` and Engineer, without manufacturing a receipt or qualification for the refused phase; the overall result remains unsuccessful, zero-fitness and nonempirical. ABI mock tests only check protocol/dispatch behavior. The exact owner's memory/swap, root mount and per-file controls are qualified separately above; aggregate workspace disk quotas remain incomplete. Owner crash/lost-response reconciliation is qualified below; no production deployment is inferred. The legacy local generated-source path remains fail-closed, and no untrusted source is compiled on the host.

The operator can attach `HubGvisorConfig` explicitly using `PapersEngine::with_hub_gvisor`, `EvolutionLoop::with_hub_gvisor` or `Engineer::with_hub_gvisor`. The normal evolution evaluator then uses the qualified Hub consumer before the legacy WASM path. Missing configuration preserves the existing fail-closed local path; configured backend errors never fall back to host compilation or structural success. The owner evidence identity is retained in evolution node results. Successful isolated execution has `isolated_runtime_diagnostic` authority and score zero; only the pre-existing explicit task oracle may establish empirical fitness. The live gate exercises this Engineer path, not merely a direct adapter call.

Researcher requests a complete executable with `main()` invoking the candidate on a bounded development example. Helper-only LLM output is refused by protocol preflight; the preserved deterministic fallback algorithms each receive a real calling harness. The live gate passes the actual generic Researcher fallback to Engineer. These self-supplied development inputs do not establish correctness or fitness; a compile-only no-op main is not substituted for a missing algorithm invocation.

Before recursive parsing, source is conservatively capped at 64 KiB and 512 ASCII punctuation bytes (including comments/strings); dense/deep grammar is refused and selects the existing fallback. The syntax preflight uses the already locked `syn` 2.0.119 parser. It requires the first statement of `main` to call a declared free function directly, with literal development inputs (or no arguments), optionally assigning its result. Empty mains, print-only statements, conditional/deferred entry calls and executable argument blocks are rejected rather than accepted from a substring. A globally budgeted 4,096-visit, 16-level body/call-chain check also refuses helpers which only log literals or stored values, including logging-only wrapper chains and unconditional early return. Budget exhaustion refuses admission, so an exponentially branching call graph cannot multiply the static walk without bound. Named formatting assignments inspect only the right-hand value. Logging arguments recursively unwrap references, groups, casts, unary wrappers, tuples and arrays; wrapping a stored value or literal is not work. Logging macro arguments can qualify only when they contain nontrivial expressions, such as an actual candidate call. The fallback driver calls an owner-generated development probe which invokes the preserved algorithm. This is a deliberately conservative syntactic activity protocol, not a semantic correctness, reachability or task-relevance proof; arbitrary generated code can still contain misleading computations. Only independent task oracles can qualify that meaning, and this diagnostic always has zero empirical fitness.

The owner's complete integrated controls archive is retained as `papers_core/qualification/hub-gvisor/owner-controls.zip`, SHA-256 `42436f60653968c1be42a198c625a9a22e690f1d569aecd2956699df3ae16eed`. CI verifies its hash, exact manifest/receipt equality and paired guest PID requests/receipts. The owner held the host task ceiling at512 in both finite probes while changing guest NPROC64→128: UID1001 produced8 processes+55 threads then EAGAIN11 at64, and8+96 without denial at128. This evidence is specific to the pinned owner runtime, not hardware performance or a new scientific verdict. The unchanged PAPERS wire omits the optional stricter guest field, so the owner applies guest NPROC equal to the requested host budget. Consumer PID configuration is now1..4096, refused before materialization/dispatch otherwise. Root-owned task workspaces fail closed; operators must use nonroot executor-owned directories. Full dependency metadata, rebuilt CLI and owner receipts must all bind the same commit, and tests/Clippy/docs run with `--locked`. The actual consumer live gate still exercises successful execution and retained refusals separately; retained owner observations do not replace it.

## Durable owner attempts adopted without semantic promotion

The owner reuses SQLite WAL/FULL commits and lifecycle_events, reserves a random
nonce/request digest before creation, commits the immutable CID before start,
and records cleaned only after exact-ID daemon absence. Its private controller
state cannot be mounted to a guest. Nonblocking safe File locks fence active
controllers; task IDs cannot be replayed even after cleanup. Uncleaned overlapping
workspaces remain quarantined, and zero/multiple/failed ownership lookups refuse.

The retained integrated archive contains two real recovery probes: controller
SIGKILL after guest entry leaves an owned running container, and lost creation
response leaves reserved state which restart binds to one nonce/digest-matched
stopped container. Read-only reconciliation never starts, replays or deletes;
CI explicitly cleans only its own exact ID before rechecking absence. These are
owner contract observations, not a production adoption, quota or empirical claim.

PAPERS pins four resolved Hub packages (core, executor, protocol, store-sqlite)
to the same revision as its rebuilt CLI and retained manifest/receipt/archive.
The live consumer separately inspects durable cleaned records for compilation,
execution, compiler-error and timeout phases. Even when the owner cleaned the
container, output admission refusal still retains the workspace and verified
compilation facts, keeps score zero and grants no scientific authority. Operator
metadata commands are `--inspect TASK_ID WORKSPACE_ROOT` and read-only
`--reconcile TASK_ID WORKSPACE_ROOT`; they return metadata, not success receipts.
Aggregate workspace OS quota and full PAPERS-01 acceptance remain incomplete.
