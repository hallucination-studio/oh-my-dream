use std::{fs, path::PathBuf};

#[test]
fn workflow_graph_domain_defines_no_row_dto_or_port_types() {
    let source_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/workflow_graph");
    for entry in fs::read_dir(source_directory).expect("workflow graph source directory") {
        let path = entry.expect("workflow graph source entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("workflow graph source should be readable");
        for forbidden_suffix in ["Row", "Dto", "Port", "Ports"] {
            assert!(
                !source.contains(forbidden_suffix),
                "{} contains forbidden boundary suffix {forbidden_suffix}",
                path.display()
            );
        }
    }
}
