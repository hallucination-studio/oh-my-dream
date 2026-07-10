use serde_json::Value;
use std::process::Command;

#[test]
fn concrete_backends_are_selected_only_by_the_application_crate() {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata should run");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid cargo metadata");
    let packages = metadata["packages"].as_array().expect("workspace packages");

    let mut normal_dependents = packages
        .iter()
        .filter(|package| package["source"].is_null())
        .filter(|package| {
            package["dependencies"].as_array().is_some_and(|dependencies| {
                dependencies.iter().any(|dependency| {
                    dependency["name"] == "backends" && dependency["kind"].is_null()
                })
            })
        })
        .map(|package| package["name"].as_str().expect("package name"))
        .collect::<Vec<_>>();
    normal_dependents.sort_unstable();

    assert_eq!(normal_dependents, ["oh-my-dream-tauri"]);
}
