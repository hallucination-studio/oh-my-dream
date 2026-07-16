use std::cmp::Ordering;

use crate::node_capability::{
    NodeCapabilityContract, NodeCapabilityContractRef,
    NodeCapabilityGenerationProfileRefParameterValue, NodeCapabilityInputBindingContract,
    NodeCapabilityInputKey, NodeCapabilityParameterKey, NodeCapabilityParameterPresence,
    NodeCapabilityParameterSet, NodeCapabilityReadinessIssue, WorkflowDataType,
    WorkflowManagedAssetIdBoundaryValue,
};
use crate::workflow_graph::{WorkflowInputBinding, WorkflowInputTarget, WorkflowNodeId};

/// One node and the structural facts needed by the pure readiness policy.
pub struct WorkflowStructuralReadinessNode<'a> {
    /// Node being checked.
    pub node_id: WorkflowNodeId,
    /// Exact resolved capability contract.
    pub contract: &'a NodeCapabilityContract,
    /// Current supplied parameters.
    pub parameters: &'a NodeCapabilityParameterSet,
    /// Current bindings targeting this node.
    pub input_bindings: &'a [(WorkflowInputTarget, WorkflowInputBinding)],
}

/// Closed pure structural readiness issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowReadinessIssue {
    /// A required capability parameter is absent.
    WorkflowRequiredParameterMissing {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Missing declared parameter.
        parameter_key: NodeCapabilityParameterKey,
    },
    /// A required single input is absent.
    WorkflowRequiredInputMissing {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Missing declared input.
        input_key: NodeCapabilityInputKey,
    },
    /// An ordered-reference input has fewer items than required.
    WorkflowReferenceMinimumNotMet {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Underfilled declared input.
        input_key: NodeCapabilityInputKey,
        /// Contract minimum.
        required_count: u32,
        /// Current item count.
        actual_count: u32,
    },
    /// A referenced managed Asset is unavailable.
    WorkflowAssetUnavailable {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Workflow target input.
        input_key: NodeCapabilityInputKey,
        /// Engine Asset-ID boundary value.
        asset_id: WorkflowManagedAssetIdBoundaryValue,
    },
    /// A referenced managed Asset has the wrong media kind.
    WorkflowAssetKindMismatch {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Workflow target input.
        input_key: NodeCapabilityInputKey,
        /// Required media kind.
        expected: WorkflowDataType,
        /// Observed media kind.
        actual: WorkflowDataType,
    },
    /// No exact capability implementation is registered.
    WorkflowCapabilityUnregistered {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Exact unresolved contract.
        capability_ref: NodeCapabilityContractRef,
    },
    /// A selected Generation Profile is incompatible with the exact capability.
    WorkflowGenerationProfileIncompatible {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Selected profile.
        profile_ref: NodeCapabilityGenerationProfileRefParameterValue,
        /// Exact capability contract.
        capability_ref: NodeCapabilityContractRef,
    },
    /// A selected Generation Profile is currently unavailable.
    WorkflowGenerationProfileUnavailable {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Selected profile.
        profile_ref: NodeCapabilityGenerationProfileRefParameterValue,
    },
    /// Availability of a selected Generation Profile is indeterminate.
    WorkflowGenerationProfileAvailabilityIndeterminate {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Selected profile.
        profile_ref: NodeCapabilityGenerationProfileRefParameterValue,
    },
    /// Any other exact capability-owned external readiness issue.
    WorkflowCapabilityExternalReadinessIssue {
        /// Owning node.
        node_id: WorkflowNodeId,
        /// Preserved typed issue.
        issue: NodeCapabilityReadinessIssue,
    },
}

impl WorkflowReadinessIssue {
    /// Encodes one issue as a frozen engine-owned boundary projection.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_readiness_issue(&mut bytes, self);
        bytes
    }

    fn node_and_tag(&self) -> (WorkflowNodeId, u8) {
        match self {
            Self::WorkflowRequiredParameterMissing { node_id, .. } => (*node_id, 0),
            Self::WorkflowRequiredInputMissing { node_id, .. } => (*node_id, 1),
            Self::WorkflowReferenceMinimumNotMet { node_id, .. } => (*node_id, 2),
            Self::WorkflowAssetUnavailable { node_id, .. } => (*node_id, 3),
            Self::WorkflowAssetKindMismatch { node_id, .. } => (*node_id, 4),
            Self::WorkflowCapabilityUnregistered { node_id, .. } => (*node_id, 5),
            Self::WorkflowGenerationProfileIncompatible { node_id, .. } => (*node_id, 6),
            Self::WorkflowGenerationProfileUnavailable { node_id, .. } => (*node_id, 7),
            Self::WorkflowGenerationProfileAvailabilityIndeterminate { node_id, .. } => {
                (*node_id, 8)
            }
            Self::WorkflowCapabilityExternalReadinessIssue { node_id, .. } => (*node_id, 9),
        }
    }

    fn compare_target(&self, other: &Self) -> Ordering {
        match (self, other) {
            (
                Self::WorkflowRequiredParameterMissing { parameter_key: left, .. },
                Self::WorkflowRequiredParameterMissing { parameter_key: right, .. },
            ) => left.cmp(right),
            (
                Self::WorkflowRequiredInputMissing { input_key: left, .. },
                Self::WorkflowRequiredInputMissing { input_key: right, .. },
            )
            | (
                Self::WorkflowReferenceMinimumNotMet { input_key: left, .. },
                Self::WorkflowReferenceMinimumNotMet { input_key: right, .. },
            )
            | (
                Self::WorkflowAssetUnavailable { input_key: left, .. },
                Self::WorkflowAssetUnavailable { input_key: right, .. },
            )
            | (
                Self::WorkflowAssetKindMismatch { input_key: left, .. },
                Self::WorkflowAssetKindMismatch { input_key: right, .. },
            ) => left.cmp(right),
            (
                Self::WorkflowCapabilityUnregistered { capability_ref: left, .. },
                Self::WorkflowCapabilityUnregistered { capability_ref: right, .. },
            ) => left.cmp(right),
            (
                Self::WorkflowGenerationProfileIncompatible { profile_ref: left, .. },
                Self::WorkflowGenerationProfileIncompatible { profile_ref: right, .. },
            )
            | (
                Self::WorkflowGenerationProfileUnavailable { profile_ref: left, .. },
                Self::WorkflowGenerationProfileUnavailable { profile_ref: right, .. },
            )
            | (
                Self::WorkflowGenerationProfileAvailabilityIndeterminate {
                    profile_ref: left, ..
                },
                Self::WorkflowGenerationProfileAvailabilityIndeterminate {
                    profile_ref: right, ..
                },
            ) => left.cmp(right),
            (
                Self::WorkflowCapabilityExternalReadinessIssue { issue: left, .. },
                Self::WorkflowCapabilityExternalReadinessIssue { issue: right, .. },
            ) => left.cmp(right),
            _ => Ordering::Equal,
        }
    }
}

fn encode_readiness_issue(bytes: &mut Vec<u8>, issue: &WorkflowReadinessIssue) {
    let (node_id, tag) = issue.node_and_tag();
    bytes.push(tag);
    bytes.extend_from_slice(node_id.as_uuid().as_bytes());
    match issue {
        WorkflowReadinessIssue::WorkflowRequiredParameterMissing { parameter_key, .. } => {
            append_text(bytes, parameter_key.as_str());
        }
        WorkflowReadinessIssue::WorkflowRequiredInputMissing { input_key, .. } => {
            append_text(bytes, input_key.as_str());
        }
        WorkflowReadinessIssue::WorkflowReferenceMinimumNotMet {
            input_key,
            required_count,
            actual_count,
            ..
        } => {
            append_text(bytes, input_key.as_str());
            bytes.extend_from_slice(&required_count.to_be_bytes());
            bytes.extend_from_slice(&actual_count.to_be_bytes());
        }
        WorkflowReadinessIssue::WorkflowAssetUnavailable { input_key, asset_id, .. } => {
            append_text(bytes, input_key.as_str());
            bytes.extend_from_slice(&asset_id.as_bytes());
        }
        WorkflowReadinessIssue::WorkflowAssetKindMismatch {
            input_key, expected, actual, ..
        } => {
            append_text(bytes, input_key.as_str());
            bytes.push(data_type_tag(*expected));
            bytes.push(data_type_tag(*actual));
        }
        WorkflowReadinessIssue::WorkflowCapabilityUnregistered { capability_ref, .. } => {
            append_contract_ref(bytes, capability_ref);
        }
        WorkflowReadinessIssue::WorkflowGenerationProfileIncompatible {
            profile_ref,
            capability_ref,
            ..
        } => {
            append_profile_ref(bytes, profile_ref);
            append_contract_ref(bytes, capability_ref);
        }
        WorkflowReadinessIssue::WorkflowGenerationProfileUnavailable { profile_ref, .. }
        | WorkflowReadinessIssue::WorkflowGenerationProfileAvailabilityIndeterminate {
            profile_ref,
            ..
        } => append_profile_ref(bytes, profile_ref),
        WorkflowReadinessIssue::WorkflowCapabilityExternalReadinessIssue { issue, .. } => {
            bytes.push(readiness_category_tag(issue.category()));
            encode_readiness_target(bytes, issue.target());
            match issue.media_kind_mismatch() {
                Some((expected, observed)) => {
                    bytes.push(1);
                    bytes.push(data_type_tag(expected));
                    bytes.push(data_type_tag(observed));
                }
                None => bytes.push(0),
            }
        }
    }
}

fn encode_readiness_target(
    bytes: &mut Vec<u8>,
    target: &crate::node_capability::NodeCapabilityReadinessTarget,
) {
    use crate::node_capability::NodeCapabilityReadinessTarget as Target;
    match target {
        Target::Capability => bytes.push(0),
        Target::ManagedAsset { parameter_key, asset_id } => {
            bytes.push(1);
            append_text(bytes, parameter_key.as_str());
            bytes.extend_from_slice(&asset_id.as_bytes());
        }
        Target::GenerationProfile { parameter_key, generation_profile_ref } => {
            bytes.push(2);
            append_text(bytes, parameter_key.as_str());
            append_profile_ref(bytes, generation_profile_ref);
        }
    }
}

fn append_contract_ref(bytes: &mut Vec<u8>, value: &NodeCapabilityContractRef) {
    append_text(bytes, value.id().as_str());
    bytes.extend_from_slice(&value.version().major().to_be_bytes());
    bytes.extend_from_slice(&value.version().minor().to_be_bytes());
}

fn append_profile_ref(
    bytes: &mut Vec<u8>,
    value: &NodeCapabilityGenerationProfileRefParameterValue,
) {
    append_text(bytes, value.profile_id());
    bytes.extend_from_slice(&value.version().to_be_bytes());
}

fn append_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

const fn data_type_tag(value: WorkflowDataType) -> u8 {
    match value {
        WorkflowDataType::Text => 0,
        WorkflowDataType::Image => 1,
        WorkflowDataType::Video => 2,
        WorkflowDataType::Audio => 3,
    }
}

const fn readiness_category_tag(
    value: crate::node_capability::NodeCapabilityReadinessCategory,
) -> u8 {
    use crate::node_capability::NodeCapabilityReadinessCategory as Category;
    match value {
        Category::InvalidCapabilityInvocation => 0,
        Category::ManagedAssetUnavailable => 1,
        Category::ManagedAssetKindMismatch => 2,
        Category::ManagedAssetReadinessIndeterminate => 3,
        Category::GenerationProfileIncompatible => 4,
        Category::GenerationProfileUnavailable => 5,
        Category::GenerationProfileAvailabilityIndeterminate => 6,
    }
}

/// Ready or a guaranteed non-empty, deterministic issue list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowReadinessResult {
    /// Pure structural requirements are satisfied.
    Ready,
    /// Structural requirements prevent admission.
    Blocked {
        /// Non-empty issues in frozen deterministic order.
        issues: Vec<WorkflowReadinessIssue>,
    },
}

impl WorkflowReadinessResult {
    /// Produces Ready for no issues or Blocked with frozen deterministic ordering.
    #[must_use]
    pub fn from_issues(mut issues: Vec<WorkflowReadinessIssue>) -> Self {
        issues.sort_by(|left, right| {
            left.node_and_tag().cmp(&right.node_and_tag()).then_with(|| left.compare_target(right))
        });
        if issues.is_empty() { Self::Ready } else { Self::Blocked { issues } }
    }
}

/// Authoritative pure structural Workflow readiness policy.
pub struct WorkflowReadinessPolicy;

impl WorkflowReadinessPolicy {
    /// Checks required parameters, inputs, and ordered-reference minimums.
    #[must_use]
    pub fn check(nodes: &[WorkflowStructuralReadinessNode<'_>]) -> WorkflowReadinessResult {
        let mut issues = Vec::new();
        for node in nodes {
            for parameter in node.contract.parameters() {
                if matches!(parameter.presence(), NodeCapabilityParameterPresence::Required)
                    && node.parameters.get(parameter.key()).is_none()
                {
                    issues.push(WorkflowReadinessIssue::WorkflowRequiredParameterMissing {
                        node_id: node.node_id,
                        parameter_key: parameter.key().clone(),
                    });
                }
            }
            for input in node.contract.inputs() {
                let binding = node.input_bindings.iter().find(|(target, _)| {
                    target.node_id == node.node_id && target.input_key == *input.key()
                });
                match input.binding() {
                    NodeCapabilityInputBindingContract::RequiredSingleValue { .. }
                        if binding.is_none() =>
                    {
                        issues.push(WorkflowReadinessIssue::WorkflowRequiredInputMissing {
                            node_id: node.node_id,
                            input_key: input.key().clone(),
                        });
                    }
                    NodeCapabilityInputBindingContract::OrderedReferences {
                        minimum_items, ..
                    } => {
                        let actual_count = binding
                            .map(|(_, binding)| binding.items().count())
                            .and_then(|count| u32::try_from(count).ok())
                            .unwrap_or(0);
                        if actual_count < *minimum_items {
                            issues.push(WorkflowReadinessIssue::WorkflowReferenceMinimumNotMet {
                                node_id: node.node_id,
                                input_key: input.key().clone(),
                                required_count: *minimum_items,
                                actual_count,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        WorkflowReadinessResult::from_issues(issues)
    }
}
