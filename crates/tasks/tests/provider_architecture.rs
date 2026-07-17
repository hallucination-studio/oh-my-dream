use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn provider_contracts_have_no_unsupported_or_support_probe_escape_hatches() {
    let source = provider_source();
    assert!(!source.contains("Unsupported"));
    assert!(!source.contains("supports_"));
    assert!(!source.contains("pub enum GenerationProviderExecution"));
    for exact_execution in [
        "TextGenerationProviderExecution",
        "ImageGenerationProviderExecution",
        "VideoGenerationProviderExecution",
        "VoiceGenerationProviderExecution",
    ] {
        assert!(source.contains(exact_execution));
    }
}

#[test]
fn provider_level_interface_has_no_generic_execution_poll_or_cancel_method() {
    let source = provider_source();
    let provider_interface = declaration_block(&source, "pub trait GenerationProviderInterface");
    assert!(!provider_interface.contains("execute"));
    assert!(!provider_interface.contains("submit"));
    assert!(!provider_interface.contains("poll"));
    assert!(!provider_interface.contains("cancel"));
}

fn declaration_block<'a>(source: &'a str, declaration: &str) -> &'a str {
    let start = source.find(declaration).unwrap();
    let remainder = &source[start..];
    let end = remainder.find("\n}").unwrap();
    &remainder[..=end]
}

fn provider_source() -> String {
    rust_sources(&manifest_dir().join("src/generation_task/interfaces/provider")).join("\n")
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
    sources.sort();
    sources
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
