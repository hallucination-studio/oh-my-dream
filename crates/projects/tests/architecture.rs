use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn project_id_has_one_typed_definition() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("projects crate is inside the workspace");
    let mut rust_files = Vec::new();
    collect_rust_files(&workspace.join("crates"), &mut rust_files);
    collect_rust_files(&workspace.join("src-tauri"), &mut rust_files);

    let declarations: Vec<PathBuf> = rust_files
        .into_iter()
        .filter(|path| {
            fs::read_to_string(path).is_ok_and(|source| {
                ["struct", "enum", "type", "as"]
                    .iter()
                    .map(|kind| format!("{kind} Project{}", "Id"))
                    .any(|declaration| source.contains(&declaration))
            })
        })
        .collect();

    assert_eq!(
        declarations,
        [workspace.join("crates/projects/src/project/domain/value.rs")],
        "ProjectId must have exactly one typed definition",
    );
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
