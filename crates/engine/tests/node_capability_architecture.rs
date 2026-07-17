use std::fs;
use std::path::{Path, PathBuf};

const ENGINE_MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn engine_capability_boundary_does_not_depend_on_outward_modules() {
    for prohibited_dependency in ["assets.workspace", "backends.workspace", "nodes.workspace"] {
        assert!(!ENGINE_MANIFEST.contains(prohibited_dependency));
    }
}

#[test]
fn node_capability_public_names_do_not_reintroduce_port_suffixes() {
    for source in rust_sources(&manifest_dir().join("src/node_capability")) {
        for identifier in public_declaration_identifiers(&source) {
            assert!(!identifier.ends_with("Port") && !identifier.ends_with("Ports"));
        }
    }
}

fn public_declaration_identifiers(source: &str) -> Vec<&str> {
    let tokens = source
        .lines()
        .filter_map(|line| line.split_once("//").map_or(Some(line), |(code, _)| Some(code)))
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>();
    tokens
        .windows(3)
        .filter(|tokens| {
            tokens[0] == "pub" && matches!(tokens[1], "struct" | "enum" | "trait" | "type")
        })
        .map(|tokens| tokens[2].trim_end_matches([':', '<', '{', '(', ';']))
        .collect()
}

fn rust_sources(root: &Path) -> Vec<String> {
    let mut pending = vec![root.to_path_buf()];
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(fs::read_to_string(path).unwrap());
            }
        }
    }
    sources
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
