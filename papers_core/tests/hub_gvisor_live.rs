use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use papers_core::evolution::{Engineer, EngineerOutput, EvaluationAuthority, Researcher};
use papers_core::hub_gvisor::{HubGvisorConfig, HubGvisorExecutor};

#[test]
#[ignore = "requires Docker with the runsc runtime and a qualified SciRust Hub binary"]
fn executes_untrusted_rust_through_qualified_hub_gvisor() {
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("missing live-test variable {name}"))
    };
    let config = HubGvisorConfig {
        executable: PathBuf::from(required("PAPERS_HUB_GVISOR_EXECUTABLE")),
        workspace_root: PathBuf::from(required("PAPERS_HUB_GVISOR_WORKSPACE_ROOT")),
        qualification_manifest: PathBuf::from(required("PAPERS_HUB_GVISOR_QUALIFICATION")),
        qualification_receipt: PathBuf::from(required("PAPERS_HUB_GVISOR_QUALIFICATION_RECEIPT")),
        image: required("PAPERS_HUB_GVISOR_IMAGE"),
        cpu_millis: 1000,
        memory_bytes: 512 * 1024 * 1024,
        pids_limit: 64,
        compile_timeout: Duration::from_secs(60),
        timeout: Duration::from_secs(120),
        max_source_bytes: 64 * 1024,
        max_capture_bytes: 64 * 1024,
    };
    let inspect = |task_id: &str| {
        let mut command = Command::new(&config.executable);
        command
            .arg("--inspect")
            .arg(task_id)
            .arg(&config.workspace_root);
        let output = hub_executor::execution_guard::execute_guarded_command(
            &mut command,
            Instant::now() + Duration::from_secs(5),
            64 * 1024,
        )
        .unwrap();
        assert!(output.status.success() && !output.timed_out);
        assert!(!output.stdout.truncated && !output.stderr.truncated);
        let record: serde_json::Value = serde_json::from_slice(&output.stdout.bytes).unwrap();
        assert_eq!(record["task_id"], task_id);
        assert_eq!(record["state"], "cleaned");
        assert_eq!(record["owned_handle"].as_str().unwrap().len(), 64);
        println!("PAPERS_HUB_DURABLE_ATTEMPT={record}");
        record
    };
    let engineer = Engineer::new(120)
        .with_hub_gvisor(config.clone())
        .expect("qualified Hub gVisor configuration must be valid");
    let candidate = Researcher::development_fallback("generic development candidate");
    let result = engineer.execute(&candidate.program, None::<fn(&str) -> EngineerOutput>);
    assert!(result.success, "{:?}", result.error);
    assert_eq!(
        result.authority,
        EvaluationAuthority::IsolatedRuntimeDiagnostic
    );
    assert_eq!(result.score, 0.0);
    assert!(!result.is_empirically_validated());
    assert_eq!(result.execution_phases.len(), 2);
    assert_eq!(result.execution_phases[0].phase, "compilation");
    assert_eq!(result.execution_phases[0].timeout_ms, 60_000);
    assert_eq!(result.execution_phases[1].phase, "execution");
    assert_eq!(result.execution_phases[1].timeout_ms, 120_000);
    assert_ne!(
        result.execution_phases[0].task_id,
        result.execution_phases[1].task_id
    );
    assert_eq!(
        result.execution_phases[0].artifact_sha256,
        result.execution_phases[1].artifact_sha256
    );
    let compilation_attempt = inspect(&result.execution_phases[0].task_id);
    let execution_attempt = inspect(&result.execution_phases[1].task_id);
    assert_ne!(compilation_attempt["attempt_id"], execution_attempt["attempt_id"]);
    println!(
        "PAPERS_GVISOR_PHASES={}",
        serde_json::to_string(&result.execution_phases).unwrap()
    );
    println!(
        "PAPERS_GVISOR_EVIDENCE={}",
        result.execution_evidence.expect("verified owner evidence")
    );
    println!("PAPERS_RESEARCHER_ENGINEER_ROUTE_OK");

    let executor = HubGvisorExecutor::new(config.clone()).unwrap();
    let refused = executor
        .execute_rust_source("fn main() { undefined_candidate_function(); }")
        .unwrap();
    assert!(!refused.success);
    assert_eq!(refused.phase_evidence.len(), 1);
    assert_eq!(refused.phase_evidence[0].phase, "compilation");
    assert!(!refused.timed_out);
    inspect(&refused.phase_evidence[0].task_id);
    println!(
        "PAPERS_GVISOR_COMPILE_REFUSAL={}",
        serde_json::to_string(&refused.phase_evidence).unwrap()
    );

    let mut deadline_config = config.clone();
    deadline_config.timeout = Duration::from_secs(3);
    let timed = HubGvisorExecutor::new(deadline_config)
        .unwrap()
        .execute_rust_source(
            r#"fn main() {
            use std::io::Write;
            println!("PAPERS_RUNTIME_ENTERED");
            std::io::stdout().flush().unwrap();
            loop { std::thread::sleep(std::time::Duration::from_millis(100)); }
        }"#,
        )
        .unwrap();
    assert!(!timed.success && timed.timed_out);
    assert_eq!(timed.phase_evidence.len(), 2);
    assert!(
        timed.stdout.contains("PAPERS_RUNTIME_ENTERED"),
        "runtime timeout must follow actual candidate entry, not just cold container startup"
    );
    assert_eq!(timed.phase_evidence[0].timeout_ms, 60_000);
    assert_eq!(timed.phase_evidence[1].timeout_ms, 3_000);
    for phase in &timed.phase_evidence {
        inspect(&phase.task_id);
    }
    println!(
        "PAPERS_GVISOR_RUNTIME_TIMEOUT={}",
        serde_json::to_string(&timed.phase_evidence).unwrap()
    );

    let workspace_root = config.workspace_root.clone();
    let prior_workspaces: Vec<_> = std::fs::read_dir(&workspace_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let mut output_config = config.clone();
    output_config.max_capture_bytes = 1024;
    let refused = Engineer::new(120)
        .with_hub_gvisor(output_config)
        .unwrap()
        .execute(
            "fn main() { for _ in 0..4096 { println!(\"bounded-output-probe\"); } }",
            None::<fn(&str) -> EngineerOutput>,
        );
    assert!(!refused.success);
    assert_eq!(refused.score, 0.0);
    assert!(!refused.is_empirically_validated());
    assert!(refused.execution_evidence.is_none());
    assert_eq!(refused.execution_phases.len(), 1);
    assert_eq!(refused.execution_phases[0].phase, "compilation");
    assert!(refused.execution_phases[0].artifact_sha256.is_some());
    let error = refused.error.unwrap();
    assert!(
        error.to_string().contains("qualified isolation contract"),
        "{error:#}"
    );
    assert!(error.to_string().contains("retained for quarantine"));
    let retained: Vec<_> = std::fs::read_dir(&workspace_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| !prior_workspaces.contains(path))
        .collect();
    assert_eq!(retained.len(), 1, "refused task must retain its workspace");
    assert!(retained[0].join("main.rs").is_file());
    assert!(retained[0].join("candidate").is_file());
    assert!(retained[0].join("compilation-request.json").is_file());
    assert!(retained[0].join("execution-request.json").is_file());
    // Process cleanup is not overall task success: admission still refuses
    // truncated output and preserves the task workspace/completed compile facts.
    inspect(&refused.execution_phases[0].task_id);
    let execution_request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(retained[0].join("execution-request.json")).unwrap(),
    )
    .unwrap();
    inspect(execution_request["task_id"].as_str().unwrap());
    println!(
        "PAPERS_GVISOR_REFUSAL_COMPLETED_PHASES={}",
        serde_json::to_string(&refused.execution_phases).unwrap()
    );
    println!("PAPERS_GVISOR_OUTPUT_LIMIT_REFUSAL_OK");
    println!("PAPERS_GVISOR_REFUSAL_QUARANTINE_OK");
}
