use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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

#[test]
fn workspace_crate_dependencies_point_toward_business_contracts() {
    let packages = workspace_packages();

    assert_eq!(workspace_dependencies(&packages, "engine"), ["projects"]);
    assert_eq!(workspace_dependencies(&packages, "assets"), ["projects"]);
    assert_eq!(workspace_dependencies(&packages, "backends"), Vec::<String>::new());
    assert_eq!(workspace_dependencies(&packages, "nodes"), ["assets", "engine"]);
    assert_eq!(
        workspace_dependencies(&packages, "oh-my-dream-tauri"),
        ["assets", "backends", "engine", "nodes"]
    );
}

#[test]
fn concrete_runtime_adapters_are_constructed_only_in_the_composition_root() {
    let root = workspace_root();
    let mut violations = Vec::new();
    collect_construction_violations(&root.join("src-tauri/src"), &mut violations);

    assert_eq!(violations, Vec::<PathBuf>::new());
}

fn workspace_packages() -> Vec<Value> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata should run");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid cargo metadata");
    metadata["packages"].as_array().expect("workspace packages").clone()
}

fn workspace_dependencies(packages: &[Value], package_name: &str) -> Vec<String> {
    let workspace_names =
        packages.iter().filter_map(|package| package["name"].as_str()).collect::<Vec<_>>();
    let package = packages
        .iter()
        .find(|package| package["name"] == package_name)
        .expect("workspace package should exist");
    let mut dependencies = package["dependencies"]
        .as_array()
        .expect("package dependencies")
        .iter()
        .filter(|dependency| dependency["kind"].is_null())
        .filter_map(|dependency| dependency["name"].as_str())
        .filter(|name| workspace_names.contains(name))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    dependencies.sort_unstable();
    dependencies
}

fn collect_construction_violations(directory: &Path, violations: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "assistant" || name == "tests") {
                continue;
            }
            collect_construction_violations(&path, violations);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path.file_name().is_none_or(|name| name != "state.rs")
            && path.file_stem().is_none_or(|name| name != "assistant")
            && path.file_stem().is_none_or(|name| name != "tests")
        {
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            if source.contains("MockBackend::new(")
                || source.contains("MockGenerationAdapter::new(")
            {
                violations.push(path);
            }
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}
