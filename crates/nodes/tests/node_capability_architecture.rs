use std::fs;
use std::path::PathBuf;

#[test]
fn active_capability_boundary_has_only_the_three_exact_provider_interfaces() {
    let provider_source =
        fs::read_to_string(manifest_dir().join("src/node_capability_media/provider.rs")).unwrap();
    let identifiers = public_declaration_identifiers(&provider_source);
    let provider_interfaces = identifiers
        .into_iter()
        .filter(|identifier| identifier.ends_with("ProviderInterface"))
        .collect::<Vec<_>>();
    assert_eq!(
        provider_interfaces,
        [
            "TextToImageProviderInterface",
            "ImageToVideoProviderInterface",
            "TextToSpeechProviderInterface",
        ]
    );
    let compact = code_without_line_comments(&provider_source).replace(char::is_whitespace, "");
    assert!(!compact.contains("BTreeMap<String"));
    assert!(!compact.contains("HashMap<String"));
    assert_eq!(
        provider_trait_method_identifiers(&provider_source),
        ["generate_image_from_text", "generate_video_from_image", "synthesize_speech_from_text",]
    );
}

#[test]
fn active_capability_boundary_has_no_second_catalog_executor_or_roadmap_registration() {
    let sources = active_capability_sources();
    let roadmap_refs = [
        "image.generate_from_image",
        "image.generate_from_reference_images",
        "image.crop",
        "video.generate_from_text",
        "video.generate_from_reference_images",
        "video.generate_from_first_frame",
        "video.generate_from_first_and_last_frames",
        "video.generate_from_mixed_media",
        "video.upscale",
        "video.extract_frames",
        "video.concatenate",
        "video.analyze_storyboard",
        "text.generate_from_text",
        "text.generate_from_mixed_media",
        "audio.generate_music_from_text",
    ];
    for source in sources {
        let code = code_without_line_comments(&source);
        let identifiers = code_identifiers(&code);
        for forbidden in ["NodeCapabilityCatalog", "NodeCapabilityExecutor", "CapabilityExecutor"] {
            assert!(!identifiers.iter().any(|identifier| identifier == &forbidden));
        }
        for roadmap_ref in roadmap_refs {
            assert!(!code.contains(roadmap_ref), "active capability contains `{roadmap_ref}`");
        }
    }
}

#[test]
fn capability_owned_modules_have_no_unsupported_or_concrete_adapter_escape_hatch() {
    for source in active_capability_sources() {
        let code = code_without_line_comments(&source);
        let identifiers = code_identifiers(&code);
        for identifier in &identifiers {
            assert!(!identifier.ends_with("Port") && !identifier.ends_with("Ports"));
            assert!(!identifier.ends_with("AdapterImpl"));
            assert_ne!(*identifier, "ProviderOptions");
        }
        assert!(!code.contains("unimplemented!"));
        assert!(!code.contains("todo!"));
    }
}

fn active_capability_sources() -> Vec<String> {
    let root = manifest_dir().join("src");
    let mut paths = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy().ends_with("_capability.rs")
                    || name == "generation_capability_execution.rs"
                    || name == "node_capability_media"
                    || name == "generation_profile"
            })
        })
        .collect::<Vec<_>>();
    let mut sources = Vec::new();
    while let Some(path) = paths.pop() {
        if path.is_dir() {
            paths.extend(fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(fs::read_to_string(path).unwrap());
        }
    }
    sources
}

fn public_declaration_identifiers(source: &str) -> Vec<String> {
    let code = code_without_line_comments(source);
    let tokens = code.split_whitespace().collect::<Vec<_>>();
    tokens
        .windows(3)
        .filter(|tokens| {
            tokens[0] == "pub" && matches!(tokens[1], "struct" | "enum" | "trait" | "type")
        })
        .map(|tokens| clean_identifier(tokens[2]))
        .collect()
}

fn provider_trait_method_identifiers(source: &str) -> Vec<String> {
    let mut in_provider_trait = false;
    let mut brace_depth = 0isize;
    let mut methods = Vec::new();
    for line in code_without_line_comments(source).lines() {
        if line.contains("pub trait") && line.contains("ProviderInterface") {
            in_provider_trait = true;
        }
        if in_provider_trait {
            let tokens = line.split_whitespace().collect::<Vec<_>>();
            if let Some(index) = tokens.iter().position(|token| *token == "fn")
                && let Some(identifier) = tokens.get(index + 1)
            {
                methods.push(clean_identifier(identifier));
            }
            brace_depth += line.matches('{').count() as isize - line.matches('}').count() as isize;
            if brace_depth == 0 && line.contains('}') {
                in_provider_trait = false;
            }
        }
    }
    methods
}

fn code_identifiers(source: &str) -> Vec<&str> {
    source
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .collect()
}

fn code_without_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

fn clean_identifier(value: &str) -> String {
    value.trim_end_matches([':', '<', '{', '(', ';']).to_owned()
}
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
