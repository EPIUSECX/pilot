use std::collections::BTreeSet;

use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("valid json")
}

fn registered_commands() -> BTreeSet<String> {
    let handler = include_str!("../src/lib.rs");
    let block = handler.split("generate_handler![").nth(1).and_then(|rest| rest.split(']').next()).expect("handler list");
    block.split(',').filter_map(|entry| entry.trim().rsplit("::").next()).filter(|name| !name.is_empty()).map(String::from).collect()
}

fn manifest_commands() -> BTreeSet<String> {
    let build = include_str!("../build.rs");
    let block = build.split("COMMANDS: &[&str] = &[").nth(1).and_then(|rest| rest.split("];").next()).expect("command list");
    block.split(',').map(|entry| entry.trim().trim_matches('"').to_string()).filter(|name| !name.is_empty()).collect()
}

#[test]
fn every_registered_command_is_in_the_acl_manifest_and_nothing_else() {
    assert_eq!(registered_commands(), manifest_commands());
}

#[test]
fn the_launcher_capability_grants_exactly_the_app_commands_to_the_main_window() {
    let capability = json(include_str!("../capabilities/launcher.json"));
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert!(capability.get("remote").is_none(), "no remote origin may receive native access");
    assert!(capability.get("webviews").is_none());

    let granted: BTreeSet<String> = capability["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    let expected: BTreeSet<String> = manifest_commands()
        .iter()
        .map(|name| format!("allow-{}", name.replace('_', "-")))
        .chain(["core:event:allow-listen".to_string(), "core:event:allow-unlisten".to_string()])
        .collect();
    assert_eq!(granted, expected);
}

#[test]
fn only_one_capability_file_exists() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let files: Vec<_> = std::fs::read_dir(directory).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")).collect();
    assert_eq!(files.len(), 1);
}

#[test]
fn the_bundle_is_sealed_with_an_ad_hoc_signature_unless_an_identity_overrides_it() {
    let configuration = json(include_str!("../tauri.conf.json"));
    assert_eq!(configuration["bundle"]["macOS"]["signingIdentity"], "-");
}

#[test]
fn the_webview_security_settings_are_not_weakened() {
    let configuration = json(include_str!("../tauri.conf.json"));
    let security = &configuration["app"]["security"];
    let csp = security["csp"].as_str().expect("a CSP is set");
    assert!(csp.contains("default-src 'self'") && !csp.contains("unsafe-eval") && !csp.contains("http:*"));
    assert!(security.get("dangerousDisableAssetCspModification").is_none());
    assert!(configuration["app"].get("withGlobalTauri").is_none_or(|value| value == false));
}
