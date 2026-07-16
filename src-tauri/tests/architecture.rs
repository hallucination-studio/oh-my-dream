use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const LIB: &str = include_str!("../src/lib.rs");
const ACTIVATED: &str = include_str!("../src/composition/activated_commands.rs");
const COMPOSITION: &str = include_str!("../src/composition.rs");
const NODE_COMPOSITION: &str = include_str!("../src/composition/node_capabilities.rs");
const ASSISTANT_COMPOSITION: &str = include_str!("../src/composition/assistant.rs");
const POST_COMMIT_EFFECT: &str = include_str!("../src/post_commit_effect/value.rs");
const NODE_PRESENTATION: &str = include_str!("../../crates/engine/src/workflow/query.rs");
const ASSISTANT_DTO: &str = include_str!("../src/assistant_command_dto.rs");
const ASSISTANT_PRESENTATION: &str = include_str!("../src/assistant_presentation.rs");
const ASSISTANT_WORKFLOW_BRIDGE: &str =
    include_str!("../src/assistant_workflow_bridge/workflow.rs");
const ASSISTANT_WORKSPACE_BRIDGE: &str =
    include_str!("../src/assistant_workflow_bridge/workspace.rs");
const FAL_TRANSPORT: &str = include_str!("../src/provider_adapters/fal.rs");
const ASSISTANT_PROCESS: &str = include_str!("../src/assistant_model_runner/process.rs");
const UI_API: &str = include_str!("../../ui/src/api/types.ts");
const UI_TAURI: &str = include_str!("../../ui/src/api/tauriApi.ts");

#[test]
fn active_boundary_has_exact_23_7_3_3_4_counts() {
    assert_eq!(registered_commands(), 23);
    assert_eq!(quoted_values(COMPOSITION, "EXACT_NODE_CAPABILITY_REFS").len(), 7);
    assert_exact_names(
        NODE_COMPOSITION,
        [
            "TextToImageProviderRouterImpl",
            "ImageToVideoProviderRouterImpl",
            "TextToSpeechProviderRouterImpl",
        ],
    );
    assert_eq!(enum_variants(POST_COMMIT_EFFECT, "DesktopPostCommitEffect").len(), 3);
    assert_eq!(enum_variants(NODE_PRESENTATION, "WorkflowNodePresentationShell").len(), 4);
}

#[test]
fn active_runtime_excludes_replaced_authorities_and_commands() {
    let active = [LIB, ACTIVATED, ASSISTANT_COMPOSITION].join("\n");
    for prohibited in [
        "AppState::from_app_handle",
        "WorkflowAuthority",
        "WorkflowPatchService",
        "assistant_send,",
        "assistant_get_pending_approval",
        "assistant_decide_approval",
        "get_assistant_config",
        "set_assistant_config",
        "get_providers",
        "set_provider_key",
    ] {
        assert!(!active.contains(prohibited), "active runtime contains {prohibited}");
    }
}

#[test]
fn assistant_boundaries_do_not_leak_secrets_paths_or_sdk_state() {
    let leak_surface = [ASSISTANT_DTO, ASSISTANT_PRESENTATION, UI_API, UI_TAURI].join("\n");
    for prohibited in [
        "api_key",
        "credential",
        "opaque_state",
        "session_path",
        "ResponsesStreamEvent",
        "RawResponsesStreamEvent",
    ] {
        assert!(!leak_surface.contains(prohibited), "boundary leak contains {prohibited}");
    }
}

#[test]
fn assistant_bridges_depend_only_on_inward_interfaces_and_use_cases() {
    let bridges = [ASSISTANT_WORKFLOW_BRIDGE, ASSISTANT_WORKSPACE_BRIDGE].join("\n");
    for prohibited in [
        "rusqlite",
        "Sqlite",
        "assistant_runtime",
        "WorkflowAuthority",
        "WorkflowPatchService",
        "LocalFilesystem",
    ] {
        assert!(!bridges.contains(prohibited), "Assistant bridge contains {prohibited}");
    }
}

#[test]
fn workspace_library_substitution_traits_use_interface_suffixes() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
    let mut violations = Vec::new();
    collect_trait_name_violations(&workspace_root.join("crates"), &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn repository_interface_implementations_use_impl_suffixes() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
    let mut violations = Vec::new();
    collect_implementation_name_violations(&workspace_root.join("crates"), &mut violations);
    collect_implementation_name_violations(&workspace_root.join("src-tauri"), &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn workspace_libraries_do_not_export_prohibited_standalone_names() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
    let mut violations = Vec::new();
    collect_prohibited_public_name_violations(&workspace_root.join("crates"), &mut violations);

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn active_private_boundary_implementations_use_impl_suffixes() {
    assert!(FAL_TRANSPORT.contains("ReqwestFalHttpTransportAdapterImpl"));
    assert!(!FAL_TRANSPORT.contains("struct ReqwestFalHttpTransport {"));
    assert!(ASSISTANT_PROCESS.contains("StdioAssistantProtocolProcessImpl"));
    assert!(!ASSISTANT_PROCESS.contains("struct StdioProtocolProcess {"));
}

fn registered_commands() -> usize {
    LIB.split("tauri::generate_handler![")
        .nth(1)
        .and_then(|value| value.split("])").next())
        .expect("Tauri handler")
        .lines()
        .filter(|line| line.trim().ends_with(','))
        .count()
}

fn quoted_values(source: &str, constant: &str) -> BTreeSet<String> {
    let body = source
        .split(constant)
        .nth(1)
        .and_then(|value| value.split("];").next())
        .expect("constant body");
    body.lines().filter_map(|line| line.split('"').nth(1)).map(str::to_owned).collect()
}

fn assert_exact_names<const N: usize>(source: &str, names: [&str; N]) {
    for name in names {
        assert_eq!(source.matches(name).count(), 3, "unexpected {name} wiring count");
    }
    assert_eq!(source.matches("ProviderRouterImpl").count(), N * 3);
}

fn enum_variants(source: &str, name: &str) -> BTreeSet<String> {
    let body = source
        .split(&format!("enum {name}"))
        .nth(1)
        .and_then(|value| value.split('}').next())
        .expect("enum body");
    body.lines()
        .filter_map(|line| {
            let value = line.trim();
            value
                .split(['(', '{', ','])
                .next()
                .filter(|item| item.chars().next().is_some_and(char::is_uppercase))
        })
        .map(str::to_owned)
        .collect()
}

fn collect_trait_name_violations(directory: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.is_dir() {
            collect_trait_name_violations(&path, violations);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs")
            || !path.components().any(|component| component.as_os_str() == "src")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("Rust source should be readable");
        collect_file_trait_name_violations(&path, &source, violations);
    }
}

fn collect_file_trait_name_violations(path: &Path, source: &str, violations: &mut Vec<String>) {
    for (index, line) in source.lines().enumerate() {
        let Some(name) = line.trim_start().strip_prefix("pub trait ").and_then(trait_name) else {
            continue;
        };
        if !name.ends_with("Interface") {
            violations.push(format!("{}:{}:{name}", path.display(), index + 1));
        }
    }
}

fn trait_name(declaration: &str) -> Option<&str> {
    declaration.split(['<', ':', ' ', '{']).next().filter(|name| !name.is_empty())
}

fn collect_implementation_name_violations(directory: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.is_dir() {
            collect_implementation_name_violations(&path, violations);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("Rust source should be readable");
        collect_file_implementation_name_violations(&path, &source, violations);
    }
}

fn collect_file_implementation_name_violations(
    path: &Path,
    source: &str,
    violations: &mut Vec<String>,
) {
    let mut declaration = String::new();
    let mut declaration_line = 0;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if declaration.is_empty() {
            if !trimmed.starts_with("impl") || !trimmed.contains("Interface") {
                continue;
            }
            declaration_line = index + 1;
        }
        declaration.push_str(trimmed);
        declaration.push(' ');
        if !trimmed.contains('{') {
            continue;
        }
        if let Some(name) = implementation_type_name(&declaration)
            && name != "Arc"
            && !name.starts_with('$')
            && !name.ends_with("Impl")
            && !name.ends_with("UseCase")
        {
            violations.push(format!("{}:{declaration_line}:{name}", path.display()));
        }
        declaration.clear();
    }
}

fn implementation_type_name(declaration: &str) -> Option<&str> {
    let target = declaration.split(" for ").nth(1)?;
    target.split(['<', ' ', '{']).next().filter(|name| !name.is_empty())
}

fn collect_prohibited_public_name_violations(directory: &Path, violations: &mut Vec<String>) {
    const PROHIBITED: [&str; 26] = [
        "Task",
        "Job",
        "Item",
        "Data",
        "Value",
        "Service",
        "Manager",
        "Handler",
        "Processor",
        "Executor",
        "Store",
        "Repository",
        "Provider",
        "Router",
        "Client",
        "Model",
        "Config",
        "Context",
        "State",
        "Result",
        "Operation",
        "Resource",
        "Content",
        "Media",
        "Registry",
        "TextToAudio",
    ];
    for entry in fs::read_dir(directory).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.is_dir() {
            collect_prohibited_public_name_violations(&path, violations);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs")
            || !path.components().any(|component| component.as_os_str() == "src")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("Rust source should be readable");
        for (index, line) in source.lines().enumerate() {
            let mut words = line.split_whitespace();
            if words.next() != Some("pub") {
                continue;
            }
            let Some(kind) = words.next() else { continue };
            if !matches!(kind, "struct" | "enum" | "trait" | "type") {
                continue;
            }
            let Some(name) = words.next().and_then(public_declaration_name) else {
                continue;
            };
            if PROHIBITED.contains(&name) {
                violations.push(format!("{}:{}:{name}", path.display(), index + 1));
            }
        }
    }
}

fn public_declaration_name(value: &str) -> Option<&str> {
    value.split(['<', ':', '=', '{']).next().filter(|name| !name.is_empty())
}
