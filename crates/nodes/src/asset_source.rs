use crate::error::{NodesError, boxed};
use crate::params::{canonicalize_mode, reject_unknown_params};
use crate::ports::output;
use crate::{AssetMediaKind, AssetReferenceRequest, AssetReferenceResolverInterface};
use engine::{
    CapabilityContract, CapabilityEffect, CapabilityPort, CapabilityPresentation, CapabilityRef,
    CapabilityRegistration, CapabilitySelector, NodeInputs, NodeInterface, NodeParams,
    NodeRunContext, NodeRunError, NodeRunResult, OutputPort, PortType, Value,
};
use std::collections::BTreeMap;
use std::sync::Arc;

const MODE: &str = "asset";

pub(crate) fn registrations(
    resolver: Arc<dyn AssetReferenceResolverInterface>,
) -> [CapabilityRegistration; 3] {
    [
        registration(
            "ImageAssetSource",
            "Image",
            "Image Asset",
            PortType::Image,
            AssetMediaKind::Image,
            Arc::clone(&resolver),
        ),
        registration(
            "VideoAssetSource",
            "Video",
            "Video Asset",
            PortType::Video,
            AssetMediaKind::Video,
            Arc::clone(&resolver),
        ),
        registration(
            "AudioAssetSource",
            "Audio",
            "Audio Asset",
            PortType::Audio,
            AssetMediaKind::Audio,
            resolver,
        ),
    ]
}

fn registration(
    type_id: &'static str,
    selector_type: &'static str,
    label: &'static str,
    port_type: PortType,
    kind: AssetMediaKind,
    resolver: Arc<dyn AssetReferenceResolverInterface>,
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
            label,
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
    resolver: Arc<dyn AssetReferenceResolverInterface>,
}

impl NodeInterface for AssetSourceNode {
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

#[cfg(test)]
mod tests {
    use super::registrations;
    use crate::{
        AssetReferenceError, AssetReferenceRequest, AssetReferenceResolverInterface,
        ResolvedAssetReference,
    };
    use engine::{Executor, NodeParams, NodeRegistry, ResultCache, Value, Workflow, WorkflowNode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct RecordingResolver(AtomicUsize);

    impl AssetReferenceResolverInterface for RecordingResolver {
        fn resolve(
            &self,
            request: AssetReferenceRequest<'_>,
        ) -> Result<ResolvedAssetReference, AssetReferenceError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ResolvedAssetReference {
                asset_id: request.asset_id.to_owned(),
                local_path: PathBuf::from("managed"),
                prompt: None,
            })
        }
    }

    #[test]
    fn asset_source_typed_outputs_and_every_run_resolution() {
        let resolver = Arc::new(RecordingResolver(AtomicUsize::new(0)));
        let mut registry = NodeRegistry::new();
        for registration in registrations(resolver.clone()) {
            registry.register_selector_capability(registration).expect("register Asset Source");
        }
        for (type_id, expected) in [
            ("Image", Value::Image("asset-1".to_owned())),
            ("Video", Value::Video("asset-1".to_owned())),
            ("Audio", Value::Audio("asset-1".to_owned())),
        ] {
            let workflow = Workflow {
                version: "1.0".to_owned(),
                project_id: "project-1".to_owned(),
                nodes: vec![WorkflowNode {
                    id: "source".to_owned(),
                    type_id: type_id.to_owned(),
                    contract_version: "1.0".to_owned(),
                    params: NodeParams::from_iter([
                        ("mode".to_owned(), serde_json::json!("asset")),
                        ("asset_id".to_owned(), serde_json::json!("asset-1")),
                    ]),
                    inputs: BTreeMap::new(),
                    position: None,
                }],
            };
            let mut cache = ResultCache::new();
            let outputs = Executor::new(&registry)
                .execute(&workflow, &mut cache)
                .expect("first Asset Source run");
            Executor::new(&registry)
                .execute(&workflow, &mut cache)
                .expect("second Asset Source run");
            let output_name = type_id.to_lowercase();
            assert_eq!(outputs["source"][&output_name], expected);
        }
        assert_eq!(resolver.0.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn asset_source_rejects_empty_ids_and_boundary_fields() {
        let resolver: Arc<dyn AssetReferenceResolverInterface> =
            Arc::new(RecordingResolver(AtomicUsize::new(0)));
        let registration = registrations(resolver).into_iter().next().expect("image registration");
        assert!(
            registration
                .normalize_params(&NodeParams::from_iter([
                    ("mode".to_owned(), serde_json::json!("asset")),
                    ("asset_id".to_owned(), serde_json::json!(""))
                ]))
                .is_err()
        );
        assert!(
            registration
                .normalize_params(&NodeParams::from_iter([
                    ("mode".to_owned(), serde_json::json!("asset")),
                    ("asset_id".to_owned(), serde_json::json!("asset-1")),
                    ("file_path".to_owned(), serde_json::json!("/tmp/private"))
                ]))
                .is_err()
        );
    }
}
