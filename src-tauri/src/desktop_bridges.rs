use std::sync::Arc;

use assets::asset::{
    application::{AssetIssuePreviewCommand, AssetIssuePreviewUseCase, AssetPreviewLease},
    domain::AssetId,
};
use async_trait::async_trait;
use engine::{
    node_capability::WorkflowNodeCapabilityRegistry,
    workflow::{
        WorkflowAggregateRepositoryInterface, WorkflowApplicationError, WorkflowGetCurrentUseCase,
        WorkflowManagedMediaPreviewSource, WorkflowMediaPreview,
        WorkflowMediaPreviewIssuerInterface, WorkflowReadinessResult,
    },
};
use projects::project::{
    application::{
        ProjectWorkflowIdBoundaryValue, ProjectWorkflowReadinessSummary,
        ProjectWorkflowRevisionBoundaryValue, ProjectWorkflowSummary,
    },
    domain::ProjectId,
    interfaces::ProjectWorkflowSummaryReaderInterface,
};

/// Same-snapshot Workflow summary bridge consumed only by Project open.
pub struct DesktopProjectWorkflowBridgeAdapterImpl<R> {
    get_current: Arc<WorkflowGetCurrentUseCase<R>>,
    capabilities: Arc<WorkflowNodeCapabilityRegistry>,
}

impl<R> DesktopProjectWorkflowBridgeAdapterImpl<R>
where
    R: WorkflowAggregateRepositoryInterface,
{
    #[must_use]
    pub const fn new(
        get_current: Arc<WorkflowGetCurrentUseCase<R>>,
        capabilities: Arc<WorkflowNodeCapabilityRegistry>,
    ) -> Self {
        Self { get_current, capabilities }
    }
}

#[async_trait]
impl<R> ProjectWorkflowSummaryReaderInterface for DesktopProjectWorkflowBridgeAdapterImpl<R>
where
    R: WorkflowAggregateRepositoryInterface + 'static,
{
    async fn read_current_project_workflow_summary(
        &self,
        project_id: ProjectId,
    ) -> Result<
        Option<ProjectWorkflowSummary>,
        projects::project::application::ProjectApplicationError,
    > {
        let current = match self
            .get_current
            .get_current_workflow_with_readiness(project_id, &self.capabilities)
            .await
        {
            Ok(current) => current,
            Err(WorkflowApplicationError::WorkflowNotFound { .. }) => return Ok(None),
            Err(_) => {
                return Err(
                    projects::project::application::ProjectApplicationError::ProjectWorkflowSummaryReadFailure,
                );
            }
        };
        let workflow_id = ProjectWorkflowIdBoundaryValue::new(
            current.workflow.id.as_uuid().hyphenated().to_string(),
        )
        .ok_or(
            projects::project::application::ProjectApplicationError::ProjectWorkflowSummaryReadFailure,
        )?;
        let workflow_revision =
            ProjectWorkflowRevisionBoundaryValue::new(current.workflow.revision.get()).ok_or(
                projects::project::application::ProjectApplicationError::ProjectWorkflowSummaryReadFailure,
            )?;
        let readiness = match current.readiness {
            WorkflowReadinessResult::Ready => ProjectWorkflowReadinessSummary::Ready,
            WorkflowReadinessResult::Blocked { .. } => ProjectWorkflowReadinessSummary::Blocked,
        };
        Ok(Some(ProjectWorkflowSummary { workflow_id, workflow_revision, readiness }))
    }
}

/// Unsigned Asset preview-lease bridge; D2 replaces its opaque encoding with the signed URI.
pub struct DesktopWorkflowMediaPreviewAdapterImpl {
    issue_preview: Arc<AssetIssuePreviewUseCase>,
}

impl DesktopWorkflowMediaPreviewAdapterImpl {
    #[must_use]
    pub const fn new(issue_preview: Arc<AssetIssuePreviewUseCase>) -> Self {
        Self { issue_preview }
    }
}

#[async_trait]
impl WorkflowMediaPreviewIssuerInterface for DesktopWorkflowMediaPreviewAdapterImpl {
    async fn issue_workflow_media_preview(
        &self,
        project_id: ProjectId,
        source: WorkflowManagedMediaPreviewSource,
    ) -> Result<WorkflowMediaPreview, WorkflowApplicationError> {
        let (asset_id, expected_fingerprint) = match source {
            WorkflowManagedMediaPreviewSource::Image(value) => {
                (value.asset_id(), value.content_fingerprint())
            }
            WorkflowManagedMediaPreviewSource::Video(value) => {
                (value.asset_id(), value.content_fingerprint())
            }
            WorkflowManagedMediaPreviewSource::Audio(value) => {
                (value.asset_id(), value.content_fingerprint())
            }
        };
        let asset_id = AssetId::from_uuid(uuid::Uuid::from_bytes(asset_id.as_bytes()))
            .map_err(|_| WorkflowApplicationError::WorkflowMediaPreviewIssueFailure)?;
        let lease = self
            .issue_preview
            .issue_asset_preview(AssetIssuePreviewCommand::new(project_id, asset_id))
            .await
            .map_err(|_| WorkflowApplicationError::WorkflowMediaPreviewIssueFailure)?;
        preview_from_lease(&lease, expected_fingerprint.as_bytes())
    }
}

fn preview_from_lease(
    lease: &AssetPreviewLease,
    expected_fingerprint: [u8; 32],
) -> Result<WorkflowMediaPreview, WorkflowApplicationError> {
    if lease.content_id().digest().as_bytes() != expected_fingerprint {
        return Err(WorkflowApplicationError::WorkflowMediaPreviewIssueFailure);
    }
    WorkflowMediaPreview::try_new(format!(
        "asset-preview-lease-v1:{}:{}:{}:{}:{}:{}",
        lease.lease_id().as_uuid(),
        lease.project_id().as_uuid(),
        lease.asset_id().as_uuid(),
        lease.content_id(),
        lease.issued_at_utc_milliseconds(),
        lease.expires_at_utc_milliseconds(),
    ))
}

#[cfg(test)]
mod tests {
    use assets::asset::{
        application::AssetPreviewLease,
        domain::{AssetContentDigest, AssetId, AssetManagedContentId, AssetPreviewLeaseId},
    };
    use projects::project::domain::ProjectId;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn unsigned_lease_conversion_is_opaque_and_rejects_stale_content() {
        let digest = [7_u8; 32];
        let lease = AssetPreviewLease::try_new(
            AssetPreviewLeaseId::from_uuid(uuid(1)).unwrap(),
            ProjectId::from_uuid(uuid(2)).unwrap(),
            AssetId::from_uuid(uuid(3)).unwrap(),
            AssetManagedContentId::from_digest(AssetContentDigest::from_bytes(digest)),
            4,
        )
        .unwrap();

        let preview = preview_from_lease(&lease, digest).unwrap();
        assert!(preview.as_str().starts_with("asset-preview-lease-v1:"));
        assert_eq!(
            preview_from_lease(&lease, [8; 32]),
            Err(WorkflowApplicationError::WorkflowMediaPreviewIssueFailure)
        );
    }

    fn uuid(seed: u8) -> Uuid {
        Uuid::from_bytes([seed, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, seed])
    }
}
