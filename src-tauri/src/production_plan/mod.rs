//! Durable Agent-owned production memory without execution semantics.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

pub mod operations;
mod service;
mod sqlite;

pub use service::{ProductionPlanRepository, ProductionPlanService};
pub use sqlite::ProductionPlanSqliteRepository;

const MAX_ITEMS: usize = 128;
const MAX_ID_BYTES: usize = 160;
const MAX_TEXT_CHARS: usize = 4_096;

/// Input used to define one user-meaningful production item.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NewPlanItem {
    pub id: String,
    pub summary: String,
}

/// Authoritative progress state for one production item.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanItemStatus {
    Pending,
    InProgress,
    Blocked,
    Completed,
}

/// One item inside a [`ProductionPlan`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlanItem {
    id: String,
    summary: String,
    status: PlanItemStatus,
    note: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoredPlanItem {
    pub id: String,
    pub summary: String,
    pub status: PlanItemStatus,
    pub note: Option<String>,
}

impl PlanItem {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn status(&self) -> PlanItemStatus {
        self.status
    }

    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Agent-owned durable plan; never an executable graph or application queue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProductionPlan {
    project_id: String,
    revision: u64,
    title: String,
    items: Vec<PlanItem>,
}

impl ProductionPlan {
    pub fn create(
        project_id: impl Into<String>,
        title: impl Into<String>,
        items: Vec<NewPlanItem>,
    ) -> Result<Self, ProductionPlanError> {
        let project_id = project_id.into();
        let title = title.into();
        validate_id("project_id", &project_id)?;
        validate_text("title", &title)?;
        let items = validate_items(items)?;
        Ok(Self { project_id, revision: 1, title, items })
    }

    pub(crate) fn restore(
        project_id: String,
        revision: u64,
        title: String,
        items: Vec<RestoredPlanItem>,
    ) -> Result<Self, ProductionPlanError> {
        validate_id("project_id", &project_id)?;
        validate_text("title", &title)?;
        if revision == 0 {
            return Err(ProductionPlanError::InvalidField { field: "revision" });
        }
        if items.is_empty() || items.len() > MAX_ITEMS {
            return Err(ProductionPlanError::InvalidItemCount { max: MAX_ITEMS });
        }
        let mut ids = HashSet::with_capacity(items.len());
        let items = items
            .into_iter()
            .map(|item| {
                validate_id("item_id", &item.id)?;
                validate_text("item_summary", &item.summary)?;
                if let Some(note) = item.note.as_deref() {
                    validate_text("item_note", note)?;
                }
                if !ids.insert(item.id.clone()) {
                    return Err(ProductionPlanError::DuplicateItemId { id: item.id });
                }
                Ok(PlanItem {
                    id: item.id,
                    summary: item.summary,
                    status: item.status,
                    note: item.note,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { project_id, revision, title, items })
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn items(&self) -> &[PlanItem] {
        &self.items
    }

    pub fn replace(
        &mut self,
        expected_revision: u64,
        title: impl Into<String>,
        items: Vec<NewPlanItem>,
    ) -> Result<(), ProductionPlanError> {
        self.check_revision(expected_revision)?;
        let title = title.into();
        validate_text("title", &title)?;
        let items = validate_items(items)?;
        let revision = next_revision(self.revision)?;
        self.title = title;
        self.items = items;
        self.revision = revision;
        Ok(())
    }

    pub fn start_item(
        &mut self,
        expected_revision: u64,
        item_id: &str,
    ) -> Result<(), ProductionPlanError> {
        self.transition_item(expected_revision, item_id, PlanItemStatus::InProgress, None)
    }

    pub fn block_item(
        &mut self,
        expected_revision: u64,
        item_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), ProductionPlanError> {
        let reason = reason.into();
        validate_text("block_reason", &reason)?;
        self.transition_item(expected_revision, item_id, PlanItemStatus::Blocked, Some(reason))
    }

    pub fn complete_item(
        &mut self,
        expected_revision: u64,
        item_id: &str,
        acceptance_note: impl Into<String>,
    ) -> Result<(), ProductionPlanError> {
        let acceptance_note = acceptance_note.into();
        validate_text("acceptance_note", &acceptance_note)?;
        self.transition_item(
            expected_revision,
            item_id,
            PlanItemStatus::Completed,
            Some(acceptance_note),
        )
    }

    fn transition_item(
        &mut self,
        expected_revision: u64,
        item_id: &str,
        next: PlanItemStatus,
        note: Option<String>,
    ) -> Result<(), ProductionPlanError> {
        self.check_revision(expected_revision)?;
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id == item_id)
            .ok_or_else(|| ProductionPlanError::ItemNotFound { id: item_id.to_owned() })?;
        if !allows_transition(item.status, next) {
            return Err(ProductionPlanError::InvalidTransition { from: item.status, to: next });
        }
        let revision = next_revision(self.revision)?;
        item.status = next;
        item.note = note;
        self.revision = revision;
        Ok(())
    }

    fn check_revision(&self, expected: u64) -> Result<(), ProductionPlanError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(ProductionPlanError::RevisionConflict { expected, actual: self.revision })
        }
    }
}

/// Domain validation or transition failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProductionPlanError {
    #[error("{field} is empty or exceeds its bound")]
    InvalidField { field: &'static str },
    #[error("production plan must contain between 1 and {max} items")]
    InvalidItemCount { max: usize },
    #[error("duplicate production plan item id `{id}`")]
    DuplicateItemId { id: String },
    #[error("production plan revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("production plan item `{id}` was not found")]
    ItemNotFound { id: String },
    #[error("production plan item cannot transition from {from:?} to {to:?}")]
    InvalidTransition { from: PlanItemStatus, to: PlanItemStatus },
    #[error("production plan revision overflow")]
    RevisionOverflow,
    #[error("production plan already exists for project `{project_id}`")]
    AlreadyExists { project_id: String },
    #[error("production plan was not found for project `{project_id}`")]
    NotFound { project_id: String },
    #[error("production plan storage failure: {message}")]
    Storage { message: String },
    #[error("invalid persisted production plan: {message}")]
    CorruptData { message: String },
}

fn validate_items(items: Vec<NewPlanItem>) -> Result<Vec<PlanItem>, ProductionPlanError> {
    if items.is_empty() || items.len() > MAX_ITEMS {
        return Err(ProductionPlanError::InvalidItemCount { max: MAX_ITEMS });
    }
    let mut ids = HashSet::with_capacity(items.len());
    items
        .into_iter()
        .map(|item| {
            validate_id("item_id", &item.id)?;
            validate_text("item_summary", &item.summary)?;
            if !ids.insert(item.id.clone()) {
                return Err(ProductionPlanError::DuplicateItemId { id: item.id });
            }
            Ok(PlanItem {
                id: item.id,
                summary: item.summary,
                status: PlanItemStatus::Pending,
                note: None,
            })
        })
        .collect()
}

fn validate_id(field: &'static str, value: &str) -> Result<(), ProductionPlanError> {
    if value.trim().is_empty() || value.len() > MAX_ID_BYTES || !value.is_ascii() {
        Err(ProductionPlanError::InvalidField { field })
    } else {
        Ok(())
    }
}

fn validate_text(field: &'static str, value: &str) -> Result<(), ProductionPlanError> {
    if value.trim().is_empty() || value.chars().count() > MAX_TEXT_CHARS {
        Err(ProductionPlanError::InvalidField { field })
    } else {
        Ok(())
    }
}

fn next_revision(current: u64) -> Result<u64, ProductionPlanError> {
    current.checked_add(1).ok_or(ProductionPlanError::RevisionOverflow)
}

fn allows_transition(from: PlanItemStatus, to: PlanItemStatus) -> bool {
    matches!(
        (from, to),
        (PlanItemStatus::Pending | PlanItemStatus::Blocked, PlanItemStatus::InProgress)
            | (PlanItemStatus::Pending | PlanItemStatus::InProgress, PlanItemStatus::Blocked)
            | (PlanItemStatus::InProgress, PlanItemStatus::Completed)
    )
}
