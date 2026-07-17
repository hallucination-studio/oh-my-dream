use nodes::GenerationProfileRef;
use tasks::generation_task::{
    GenerationProviderContract, GenerationProviderId, GenerationProviderRouteId,
    GenerationTaskRequestKind,
};

/// Non-zero monotonic Settings revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsRevision(u64);

impl GenerationProviderSettingsRevision {
    /// Creates a non-zero revision.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Returns the revision integer.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One complete active `(profile, kind)` provider/route selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsBinding {
    profile_ref: GenerationProfileRef,
    generation_kind: GenerationTaskRequestKind,
    provider_id: GenerationProviderId,
    route_id: GenerationProviderRouteId,
}

impl GenerationProviderSettingsBinding {
    /// Creates one indivisible binding.
    #[must_use]
    pub const fn new(
        profile_ref: GenerationProfileRef,
        generation_kind: GenerationTaskRequestKind,
        provider_id: GenerationProviderId,
        route_id: GenerationProviderRouteId,
    ) -> Self {
        Self { profile_ref, generation_kind, provider_id, route_id }
    }

    /// Returns the selected profile.
    #[must_use]
    pub const fn profile_ref(&self) -> &GenerationProfileRef {
        &self.profile_ref
    }
    /// Returns the selected generation kind.
    #[must_use]
    pub const fn generation_kind(&self) -> GenerationTaskRequestKind {
        self.generation_kind
    }
    /// Returns the selected provider.
    #[must_use]
    pub const fn provider_id(&self) -> &GenerationProviderId {
        &self.provider_id
    }
    /// Returns the selected route.
    #[must_use]
    pub const fn route_id(&self) -> &GenerationProviderRouteId {
        &self.route_id
    }
}

/// Persisted sanitized Settings state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsSnapshot {
    revision: GenerationProviderSettingsRevision,
    bindings: Vec<GenerationProviderSettingsBinding>,
}

impl GenerationProviderSettingsSnapshot {
    /// Restores one sorted unique snapshot.
    pub fn try_new(
        revision: GenerationProviderSettingsRevision,
        mut bindings: Vec<GenerationProviderSettingsBinding>,
    ) -> Result<Self, GenerationProviderSettingsError> {
        bindings.sort_by(compare_bindings);
        if bindings.windows(2).any(|pair| compare_bindings(&pair[0], &pair[1]).is_eq()) {
            return Err(GenerationProviderSettingsError::InvalidSnapshot);
        }
        Ok(Self { revision, bindings })
    }

    /// Returns the current revision.
    #[must_use]
    pub const fn revision(&self) -> GenerationProviderSettingsRevision {
        self.revision
    }
    /// Returns bindings in profile/kind order.
    #[must_use]
    pub fn bindings(&self) -> &[GenerationProviderSettingsBinding] {
        &self.bindings
    }
}

/// One validated Settings mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationProviderSettingsMutation {
    /// Selects one exact safe contract route.
    SetBinding(GenerationProviderSettingsBinding),
    /// Removes one exact profile/kind selection.
    RemoveBinding {
        /// Exact profile key.
        profile_ref: GenerationProfileRef,
        /// Exact generation kind key.
        generation_kind: GenerationTaskRequestKind,
    },
}

/// Atomic repository mutation result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenerationProviderSettingsMutationResult {
    /// State changed and revision advanced once.
    Committed(GenerationProviderSettingsSnapshot),
    /// Requested state already held; revision did not change.
    Unchanged(GenerationProviderSettingsSnapshot),
    /// Expected revision was stale.
    RevisionConflict,
}

/// Safe route choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsRouteChoice {
    /// Stable route identity.
    pub route_id: GenerationProviderRouteId,
    /// Safe display name.
    pub display_name: String,
}

/// Safe provider choice with non-empty compatible routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsProviderChoice {
    /// Stable provider identity.
    pub provider_id: GenerationProviderId,
    /// Safe display name.
    pub display_name: String,
    /// Compatible routes in route-ID order.
    pub routes: Vec<GenerationProviderSettingsRouteChoice>,
}

/// One selectable profile/kind item and its optional current binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsProfile {
    /// Exact profile identity.
    pub profile_ref: GenerationProfileRef,
    /// Exact generation kind.
    pub generation_kind: GenerationTaskRequestKind,
    /// Current complete binding, when selected.
    pub selected_binding: Option<GenerationProviderSettingsBinding>,
    /// Structurally compatible choices.
    pub provider_choices: Vec<GenerationProviderSettingsProviderChoice>,
}

/// Complete sanitized Settings read model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenerationProviderSettingsView {
    /// Current persisted revision.
    pub settings_revision: GenerationProviderSettingsRevision,
    /// Profile/kind items in stable order.
    pub profiles: Vec<GenerationProviderSettingsProfile>,
}

/// Closed Settings application failures.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum GenerationProviderSettingsError {
    /// Persisted state violated the Settings contract.
    #[error("Generation Provider Settings snapshot is invalid")]
    InvalidSnapshot,
    /// A mutation selected a tuple absent from the safe provider contracts.
    #[error("Generation Provider Settings mutation is invalid")]
    InvalidMutation,
    /// The expected Settings revision is stale.
    #[error("Generation Provider Settings revision conflict")]
    RevisionConflict,
    /// Settings persistence failed.
    #[error("Generation Provider Settings storage failed")]
    Repository,
    /// Safe provider contracts were inconsistent.
    #[error("Generation Provider Settings contract projection failed")]
    ContractProjection,
}

pub(crate) fn focused_routes(
    contract: &GenerationProviderContract,
    kind: GenerationTaskRequestKind,
) -> Option<&[tasks::generation_task::GenerationProviderRouteContract]> {
    match kind {
        GenerationTaskRequestKind::Text => contract.text().map(|value| value.routes()),
        GenerationTaskRequestKind::Image => contract.image().map(|value| value.routes()),
        GenerationTaskRequestKind::Video => contract.video().map(|value| value.routes()),
        GenerationTaskRequestKind::Voice => contract.voice().map(|value| value.routes()),
    }
}

fn compare_bindings(
    left: &GenerationProviderSettingsBinding,
    right: &GenerationProviderSettingsBinding,
) -> std::cmp::Ordering {
    left.profile_ref
        .cmp(&right.profile_ref)
        .then_with(|| kind_order(left.generation_kind).cmp(&kind_order(right.generation_kind)))
}

pub(crate) const fn kind_order(kind: GenerationTaskRequestKind) -> u8 {
    match kind {
        GenerationTaskRequestKind::Text => 0,
        GenerationTaskRequestKind::Image => 1,
        GenerationTaskRequestKind::Video => 2,
        GenerationTaskRequestKind::Voice => 3,
    }
}
