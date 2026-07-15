const ENGINE_MANIFEST: &str = include_str!("../Cargo.toml");
const NODE_CAPABILITY_SOURCES: &[&str] = &[
    include_str!("../src/node_capability/boundary_value.rs"),
    include_str!("../src/node_capability/contract.rs"),
    include_str!("../src/node_capability/contract_error.rs"),
    include_str!("../src/node_capability/error.rs"),
    include_str!("../src/node_capability/execution.rs"),
    include_str!("../src/node_capability/identity.rs"),
    include_str!("../src/node_capability/interface.rs"),
    include_str!("../src/node_capability/normalization.rs"),
    include_str!("../src/node_capability/parameter.rs"),
    include_str!("../src/node_capability/registry.rs"),
    include_str!("../src/node_capability/runtime_value.rs"),
];

#[test]
fn engine_capability_boundary_does_not_depend_on_outward_modules() {
    for prohibited_dependency in ["assets.workspace", "backends.workspace", "nodes.workspace"] {
        assert!(!ENGINE_MANIFEST.contains(prohibited_dependency));
    }
}

#[test]
fn node_capability_public_names_do_not_reintroduce_port_suffixes() {
    for source in NODE_CAPABILITY_SOURCES {
        assert!(!source.contains("Port"));
    }
}
