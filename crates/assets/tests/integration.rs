// Original integration modules intentionally own isolated copies of shared test fixtures.
#![allow(clippy::duplicate_mod)]

#[path = "asset_aggregate_transitions.rs"]
mod asset_aggregate_transitions;
#[path = "asset_domain_values.rs"]
mod asset_domain_values;
#[path = "asset_finalize_content_use_case.rs"]
mod asset_finalize_content_use_case;
#[path = "asset_import_use_case.rs"]
mod asset_import_use_case;
#[path = "asset_interface_values.rs"]
mod asset_interface_values;
#[path = "asset_interfaces.rs"]
mod asset_interfaces;
#[path = "asset_node_output_values.rs"]
mod asset_node_output_values;
#[path = "asset_orchestration_values.rs"]
mod asset_orchestration_values;
#[path = "asset_origin_values.rs"]
mod asset_origin_values;
#[path = "asset_query_use_cases.rs"]
mod asset_query_use_cases;
#[path = "asset_reconcile_content_use_case.rs"]
mod asset_reconcile_content_use_case;
#[path = "asset_record_node_output_use_case.rs"]
mod asset_record_node_output_use_case;
#[path = "integrity.rs"]
mod integrity;
#[path = "store.rs"]
mod store;
