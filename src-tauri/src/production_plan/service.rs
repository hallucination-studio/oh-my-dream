use super::{NewPlanItem, ProductionPlan, ProductionPlanError};
use std::sync::Arc;

/// Persistence boundary consumed by the production-plan capability.
pub trait ProductionPlanRepository: Send + Sync {
    fn load(&self, project_id: &str) -> Result<Option<ProductionPlan>, ProductionPlanError>;
    fn insert(&self, plan: &ProductionPlan) -> Result<(), ProductionPlanError>;
    fn update(
        &self,
        expected_revision: u64,
        plan: &ProductionPlan,
    ) -> Result<(), ProductionPlanError>;
}

/// Application service that owns durable production-plan use cases.
pub struct ProductionPlanService {
    repository: Arc<dyn ProductionPlanRepository>,
}

impl ProductionPlanService {
    #[must_use]
    pub fn new(repository: Arc<dyn ProductionPlanRepository>) -> Self {
        Self { repository }
    }

    pub fn get(&self, project_id: &str) -> Result<Option<ProductionPlan>, ProductionPlanError> {
        self.repository.load(project_id)
    }

    pub fn create(
        &self,
        project_id: &str,
        title: String,
        items: Vec<NewPlanItem>,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        let plan = ProductionPlan::create(project_id, title, items)?;
        self.repository.insert(&plan)?;
        Ok(plan)
    }

    pub fn replace(
        &self,
        project_id: &str,
        expected_revision: u64,
        title: String,
        items: Vec<NewPlanItem>,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        self.mutate(project_id, expected_revision, |plan| {
            plan.replace(expected_revision, title, items)
        })
    }

    pub fn start_item(
        &self,
        project_id: &str,
        expected_revision: u64,
        item_id: &str,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        self.mutate(project_id, expected_revision, |plan| {
            plan.start_item(expected_revision, item_id)
        })
    }

    pub fn block_item(
        &self,
        project_id: &str,
        expected_revision: u64,
        item_id: &str,
        reason: String,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        self.mutate(project_id, expected_revision, |plan| {
            plan.block_item(expected_revision, item_id, reason)
        })
    }

    pub fn complete_item(
        &self,
        project_id: &str,
        expected_revision: u64,
        item_id: &str,
        note: String,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        self.mutate(project_id, expected_revision, |plan| {
            plan.complete_item(expected_revision, item_id, note)
        })
    }

    fn mutate(
        &self,
        project_id: &str,
        expected_revision: u64,
        mutation: impl FnOnce(&mut ProductionPlan) -> Result<(), ProductionPlanError>,
    ) -> Result<ProductionPlan, ProductionPlanError> {
        let mut plan = self
            .repository
            .load(project_id)?
            .ok_or_else(|| ProductionPlanError::NotFound { project_id: project_id.to_owned() })?;
        mutation(&mut plan)?;
        self.repository.update(expected_revision, &plan)?;
        Ok(plan)
    }
}
