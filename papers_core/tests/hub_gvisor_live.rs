use std::path::PathBuf;
use std::time::Duration;

use papers_core::evolution::{Engineer, EngineerOutput, EvaluationAuthority, Researcher};
use papers_core::hub_gvisor::HubGvisorConfig;

#[test]
#[ignore = "requires Docker with the runsc runtime and a qualified SciRust Hub binary"]
fn executes_untrusted_rust_through_qualified_hub_gvisor() {
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("missing live-test variable {name}"))
    };
    let engineer = Engineer::new(120)
        .with_hub_gvisor(HubGvisorConfig {
            executable: PathBuf::from(required("PAPERS_HUB_GVISOR_EXECUTABLE")),
            workspace_root: PathBuf::from(required("PAPERS_HUB_GVISOR_WORKSPACE_ROOT")),
            qualification_manifest: PathBuf::from(required("PAPERS_HUB_GVISOR_QUALIFICATION")),
            qualification_receipt: PathBuf::from(required(
                "PAPERS_HUB_GVISOR_QUALIFICATION_RECEIPT",
            )),
            image: required("PAPERS_HUB_GVISOR_IMAGE"),
            cpu_millis: 1000,
            memory_bytes: 512 * 1024 * 1024,
            pids_limit: 64,
            timeout: Duration::from_secs(120),
            max_source_bytes: 64 * 1024,
            max_capture_bytes: 64 * 1024,
        })
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
    println!(
        "PAPERS_GVISOR_EVIDENCE={}",
        result.execution_evidence.expect("verified owner evidence")
    );
    println!("PAPERS_RESEARCHER_ENGINEER_ROUTE_OK");
}
