use std::path::PathBuf;
use std::time::Duration;

use papers_core::hub_gvisor::{HubGvisorConfig, HubGvisorExecutor};

#[test]
#[ignore = "requires Docker with the runsc runtime and a qualified SciRust Hub binary"]
fn executes_untrusted_rust_through_qualified_hub_gvisor() {
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("missing live-test variable {name}"))
    };
    let executor = HubGvisorExecutor::new(HubGvisorConfig {
        executable: PathBuf::from(required("PAPERS_HUB_GVISOR_EXECUTABLE")),
        workspace_root: PathBuf::from(required("PAPERS_HUB_GVISOR_WORKSPACE_ROOT")),
        qualification_manifest: PathBuf::from(required(
            "PAPERS_HUB_GVISOR_QUALIFICATION",
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
    let result = executor
        .execute_rust_source(
            r#"fn main() {
    println!("PAPERS_HUB_GVISOR_LIVE_OK");
}"#,
        )
        .expect("qualified Hub gVisor execution must produce a typed receipt");
    assert!(result.success, "{:?}", result.stderr);
    assert!(result.stdout.contains("PAPERS_HUB_GVISOR_LIVE_OK"));
    println!("PAPERS_GVISOR_EVIDENCE={}", result.isolation_evidence_id);
}
