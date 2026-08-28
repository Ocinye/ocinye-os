//! End-to-end tests of the SystemCapability Runtime against a real WebAssembly
//! component.
//!
//! These load `wasm/capabilities/bibtex-import` compiled to `wasm32-wasip1` and
//! run it under the host.
//!
//! # Why a missing component fails rather than skips
//!
//! An earlier version skipped when the artefact was absent. Because cargo hides
//! the output of passing tests, a build script that produced the component in
//! the wrong directory made all four tests skip *and report success* — and two
//! real defects survived behind that green result.
//!
//! Failing costs one command on a fresh clone. Skipping cost real coverage
//! without anyone noticing, which is exactly what `CLAUDE.md` §70 forbids.

use std::path::PathBuf;

use ocinye_capabilities::{CapabilityError, CapabilityRuntime, Invocation, Manifest};

const SAMPLE_BIBTEX: &str = r#"
@article{mucai2024,
  title   = {Wind resource assessment for {Angola}},
  author  = {Mucai, Ana and Silva, João P.},
  journal = {Renewable Energy},
  year    = {2024},
  doi     = {10.1016/j.renene.2024.01.001}
}
"#;

/// Onde está o componente compilado.
///
/// `OCINYE_TEST_CAPABILITY_WASM` aponta-o, tal como `OCINYE_TEST_CHROME` aponta
/// o browser das viagens. Chamava-se `OCINYE_CAPABILITY_WASM` e o prefixo dizia
/// que era configuração de uma instalação, quando é de harness — e o guarda da
/// superfície de configuração exigia, com razão, que aparecesse no
/// `.env.example` de quem instala o sistema.
fn component_path() -> PathBuf {
    std::env::var("OCINYE_TEST_CAPABILITY_WASM").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/wasm32-wasip1/release/bibtex-import.wasm")
        },
        PathBuf::from,
    )
}

fn load_component() -> Vec<u8> {
    let path = component_path();
    std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "the WebAssembly component is not built.\n\
             Expected: {}\n\
             Run: ./scripts/build-capabilities.sh",
            path.display()
        )
    })
}

fn manifest() -> Manifest {
    let raw = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../wasm/capabilities/bibtex-import/manifest.json"),
    )
    .expect("the example capability ships a manifest");
    Manifest::parse(&raw).expect("the example manifest is valid")
}

#[test]
fn a_real_capability_runs_and_returns_structured_output() {
    let component = load_component();
    let manifest = manifest();
    let runtime = CapabilityRuntime::new().expect("runtime");

    let outcome = runtime
        .run(Invocation {
            manifest: &manifest,
            component: &component,
            input: SAMPLE_BIBTEX.as_bytes().to_vec(),
        })
        .expect("the capability should run");

    let parsed: serde_json::Value =
        serde_json::from_slice(&outcome.output).expect("output should be JSON");

    assert_eq!(parsed["sources"][0]["citation_key"], "mucai2024");
    assert_eq!(parsed["sources"][0]["year"], 2024);
    assert_eq!(parsed["sources"][0]["authors"][0], "Mucai, Ana");
    assert!(parsed["skipped"].as_array().is_some_and(Vec::is_empty));

    assert!(
        outcome.fuel_used.is_some_and(|fuel| fuel > 0),
        "fuel should be metered"
    );
    assert!(outcome.duration.as_millis() < u128::from(manifest.limits.wall_time_ms));
}

#[test]
fn a_capability_starved_of_fuel_is_stopped_rather_than_left_running() {
    let component = load_component();

    // One unit of fuel cannot complete any real work: this proves the limit is
    // enforced, not merely configured.
    let mut manifest = manifest();
    manifest.limits.fuel = 1;

    let runtime = CapabilityRuntime::new().expect("runtime");
    let result = runtime.run(Invocation {
        manifest: &manifest,
        component: &component,
        input: SAMPLE_BIBTEX.as_bytes().to_vec(),
    });

    assert!(
        matches!(result, Err(CapabilityError::ResourceExhausted(_))),
        "expected the fuel limit to stop the capability, got {result:?}"
    );
}

#[test]
fn a_capability_gets_no_environment_it_did_not_ask_for() {
    let component = load_component();

    // The host sets no environment variables and preopens no directory. If the
    // capability could read the host environment, a secret in it would be
    // readable by untrusted code.
    std::env::set_var("OCINYE_TEST_SECRET", "must-not-be-visible");

    let outcome = CapabilityRuntime::new()
        .expect("runtime")
        .run(Invocation {
            manifest: &manifest(),
            component: &component,
            input: SAMPLE_BIBTEX.as_bytes().to_vec(),
        })
        .expect("the capability should run");

    let rendered = String::from_utf8_lossy(&outcome.output);
    assert!(!rendered.contains("must-not-be-visible"));
    assert!(!outcome.diagnostics.contains("must-not-be-visible"));
}

#[test]
fn malformed_input_is_reported_by_the_capability_not_by_a_crash() {
    let component = load_component();

    let outcome = CapabilityRuntime::new()
        .expect("runtime")
        .run(Invocation {
            manifest: &manifest(),
            component: &component,
            input: b"@{ this is not a valid entry".to_vec(),
        })
        .expect("the capability should still run");

    let parsed: serde_json::Value = serde_json::from_slice(&outcome.output).expect("JSON");
    assert!(parsed["sources"].as_array().is_some_and(Vec::is_empty));
    assert!(parsed["skipped"]
        .as_array()
        .is_some_and(|skipped| !skipped.is_empty()));
}
