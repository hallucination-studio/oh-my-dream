use crate::dto::CapabilityDto;
use serde_json::json;

/// Backend capabilities exposed to the assistant.
#[must_use]
pub fn backend_capabilities() -> Vec<CapabilityDto> {
    vec![
        capability("workflow.run", "Run the current workflow.", "run_workflow", true),
        capability("asset.list", "List stored assets with optional filters.", "list_assets", false),
        capability("asset.get", "Fetch one stored asset by id.", "get_asset", false),
        capability("project.list", "List local projects.", "list_projects", false),
        capability("project.create", "Create a local project.", "create_project", false),
        capability("project.open", "Open a project and its saved workflow.", "open_project", false),
        capability("workflow.save", "Persist the current workflow JSON.", "save_workflow", true),
        capability("workflow.load", "Load workflow JSON for a project.", "load_workflow", false),
        capability("provider.list", "List generation providers.", "get_providers", false),
        capability(
            "provider.set_active",
            "Set the active generation provider.",
            "set_active_provider",
            false,
        ),
        capability(
            "provider.set_key",
            "Store a provider API key locally.",
            "set_provider_key",
            true,
        ),
        capability(
            "assistant.get_config",
            "Read assistant settings.",
            "get_assistant_config",
            false,
        ),
        capability(
            "assistant.set_config",
            "Update assistant settings.",
            "set_assistant_config",
            true,
        ),
        capability("skill.list", "List installed assistant skills.", "list_skills", false),
        capability(
            "skill.install",
            "Install a local declarative assistant skill.",
            "install_skill",
            true,
        ),
        capability(
            "skill.set_enabled",
            "Enable or disable an installed skill.",
            "set_skill_enabled",
            true,
        ),
        capability("skill.uninstall", "Uninstall an assistant skill.", "uninstall_skill", true),
    ]
}

fn capability(name: &str, description: &str, command: &str, confirm: bool) -> CapabilityDto {
    CapabilityDto {
        name: name.to_owned(),
        description: description.to_owned(),
        kind: "backend".to_owned(),
        command: Some(command.to_owned()),
        parameters: json!({ "type": "object", "properties": {}, "additionalProperties": true }),
        returns: json!({ "type": "object" }),
        confirm,
    }
}
