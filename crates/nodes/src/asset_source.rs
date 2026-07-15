use crate::error::{NodesError, boxed};
use crate::params::{canonicalize_mode, reject_unknown_params};
use crate::ports::output;
use crate::{AssetMediaKind, AssetReferenceRequest, AssetReferenceResolver};
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, Node, NodeInputs, NodeParams, NodeRunContext,
    NodeRunError, NodeRunResult, OutputPort, PortType, Value,
};
use std::collections::BTreeMap;
use std::sync::Arc;

const MODE: &str = "asset";

pub(crate) fn registrations(
    resolver: Arc<dyn AssetReferenceResolver>,
) -> [CapabilityRegistration; 3] {
    [
        registration(
            "ImageAssetSource",
            "Image",
            PortType::Image,
            AssetMediaKind::Image,
            Arc::clone(&resolver),
        ),
        registration(
            "VideoAssetSource",
            "Video",
            PortType::Video,
            AssetMediaKind::Video,
            Arc::clone(&resolver),
        ),
        registration("AudioAssetSource", "Audio", PortType::Audio, AssetMediaKind::Audio, resolver),
    ]
}

fn registration(
    type_id: &'static str,
    selector_type: &'static str,
    port_type: PortType,
    kind: AssetMediaKind,
    resolver: Arc<dyn AssetReferenceResolver>,
) -> CapabilityRegistration {
    let contract = CapabilityContract::contextual(
        CapabilityRef::new(type_id, engine::DEFAULT_CAPABILITY_VERSION),
        Vec::new(),
        vec![CapabilityPort::output(output_name(kind), port_type)],
        serde_json::json!({
            "type": "object",
            "required": ["mode", "asset_id"],
            "properties": {
                "mode": {"type": "string", "const": MODE},
                "asset_id": {"type": "string", "minLength": 1}
            },
            "additionalProperties": false
        }),
        "asset_library",
        vec![CapabilityEffect::LocalRead],
    );
    CapabilityRegistration::new(
        contract,
        CapabilityPresentation::new(
            type_id,
            "Reuse a managed local Asset.",
            "assets",
            vec!["asset".to_owned(), selector_type.to_lowercase()],
        ),
        Box::new(normalize_params),
        Box::new(move |params| {
            let asset_id = params
                .get("asset_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| boxed(NodesError::MissingInput { name: "asset_id".to_owned() }))?;
            Ok(Box::new(AssetSourceNode {
                type_id,
                asset_id: asset_id.to_owned(),
                kind,
                outputs: vec![output(output_name(kind), port_type)],
                resolver: Arc::clone(&resolver),
            }))
        }),
    )
    .with_selector(CapabilitySelector::new(selector_type, MODE))
}

fn normalize_params(params: &NodeParams) -> Result<NodeParams, NodeRunError> {
    reject_unknown_params(params, &["mode", "asset_id"]).map_err(boxed)?;
    let asset_id = params
        .get("asset_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            boxed(NodesError::InvalidParam {
                name: "asset_id".to_owned(),
                reason: "must be a non-empty stable Asset ID".to_owned(),
            })
        })?;
    let mut normalized = NodeParams::from_iter([(
        "asset_id".to_owned(),
        serde_json::Value::String(asset_id.to_owned()),
    )]);
    canonicalize_mode(params, &mut normalized, MODE).map_err(boxed)?;
    Ok(normalized)
}

struct AssetSourceNode {
    type_id: &'static str,
    asset_id: String,
    kind: AssetMediaKind,
    outputs: Vec<OutputPort>,
    resolver: Arc<dyn AssetReferenceResolver>,
}

impl Node for AssetSourceNode {
    fn type_id(&self) -> &str {
        self.type_id
    }
    fn inputs(&self) -> &[engine::InputPort] {
        &[]
    }
    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }
    fn run(
        &self,
        _inputs: &NodeInputs,
        context: &mut NodeRunContext,
    ) -> Result<NodeRunResult, NodeRunError> {
        self.resolver
            .resolve(AssetReferenceRequest {
                project_id: context.project_id(),
                asset_id: &self.asset_id,
                expected_kind: self.kind,
            })
            .map_err(|error| Box::new(error) as NodeRunError)?;
        let value = match self.kind {
            AssetMediaKind::Image => Value::Image(self.asset_id.clone()),
            AssetMediaKind::Video => Value::Video(self.asset_id.clone()),
            AssetMediaKind::Audio => Value::Audio(self.asset_id.clone()),
        };
        Ok(NodeRunResult::new(BTreeMap::from([(output_name(self.kind).to_owned(), value)])))
    }
}

fn output_name(kind: AssetMediaKind) -> &'static str {
    match kind {
        AssetMediaKind::Image => "image",
        AssetMediaKind::Video => "video",
        AssetMediaKind::Audio => "audio",
    }
}
