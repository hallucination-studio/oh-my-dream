// Original integration modules intentionally own isolated copies of shared test fixtures.
#![allow(clippy::duplicate_mod)]

#[path = "architecture.rs"]
mod architecture;
#[path = "project_domain.rs"]
mod project_domain;
#[path = "project_interfaces.rs"]
mod project_interfaces;
#[path = "project_use_cases.rs"]
mod project_use_cases;
