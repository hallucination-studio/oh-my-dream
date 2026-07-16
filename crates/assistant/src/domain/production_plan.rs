//! Durable Assistant production-plan working memory.

use std::collections::BTreeSet;

use projects::project::domain::ProjectId;

use super::{AssistantProductionPlanId, AssistantSessionId};

/// Production-plan invariant or transition failure.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssistantProductionPlanError {
    /// A bounded text value is empty or exceeds its scalar limit.
    #[error("Assistant production plan text is invalid")]
    InvalidText,
    /// An item identity violates its frozen grammar.
    #[error("Assistant production plan item identity is invalid")]
    InvalidItemId,
    /// More than 128 items were supplied.
    #[error("Assistant production plan has too many items")]
    TooManyItems,
    /// Two items have the same plan-local identity.
    #[error("Assistant production plan contains a duplicate item identity")]
    DuplicateItemId,
    /// The optimistic revision differs from the aggregate revision.
    #[error("Assistant production plan revision conflict")]
    RevisionConflict {
        /// Revision supplied by the caller.
        expected: u64,
        /// Current aggregate revision.
        actual: u64,
    },
    /// The requested item does not exist.
    #[error("Assistant production plan item was not found")]
    ItemNotFound,
    /// The item state does not permit the requested transition.
    #[error("Assistant production plan item transition is invalid")]
    InvalidItemTransition,
    /// Revision increment overflowed.
    #[error("Assistant production plan revision overflow")]
    RevisionOverflow,
    /// A restored revision is zero.
    #[error("Assistant production plan revision is invalid")]
    InvalidRevision,
}

macro_rules! bounded_text {
    ($name:ident, $max:expr, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug, Hash, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Normalizes and validates the bounded text.
            pub fn new(value: impl AsRef<str>) -> Result<Self, AssistantProductionPlanError> {
                let value = value.as_ref().trim();
                if value.is_empty() || value.chars().count() > $max {
                    Err(AssistantProductionPlanError::InvalidText)
                } else {
                    Ok(Self(value.to_owned()))
                }
            }

            /// Returns the normalized text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

bounded_text!(AssistantPlanTitle, 120, "Bounded production-plan title.");
bounded_text!(AssistantPlanItemGoal, 2_000, "Bounded user-meaningful plan-item goal.");
bounded_text!(
    AssistantPlanItemAcceptanceNote,
    2_000,
    "Bounded evidence recorded when a plan item completes."
);
bounded_text!(
    AssistantPlanItemBlockedReason,
    1_000,
    "Bounded reason recorded while a plan item is blocked."
);

/// Stable identity of one item inside a plan.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssistantPlanItemId(String);

impl AssistantPlanItemId {
    /// Validates the frozen plan-local key grammar.
    pub fn new(value: impl Into<String>) -> Result<Self, AssistantProductionPlanError> {
        let value = value.into();
        let mut bytes = value.bytes();
        let valid = (1..=64).contains(&value.len())
            && matches!(bytes.next(), Some(b'a'..=b'z'))
            && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if valid { Ok(Self(value)) } else { Err(AssistantProductionPlanError::InvalidItemId) }
    }

    /// Returns the stable plan-local key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed state of one production-plan item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistantPlanItemState {
    /// Work has not started.
    Pending,
    /// Work is currently being discussed or performed.
    InProgress,
    /// Work cannot proceed for the recorded reason.
    Blocked {
        /// Exact bounded blocking reason.
        reason: AssistantPlanItemBlockedReason,
    },
    /// Work completed with the recorded acceptance evidence.
    Completed {
        /// Exact bounded acceptance note.
        acceptance_note: AssistantPlanItemAcceptanceNote,
    },
}

/// One user-meaningful item inside a production plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantPlanItemEntity {
    id: AssistantPlanItemId,
    goal: AssistantPlanItemGoal,
    state: AssistantPlanItemState,
}

impl AssistantPlanItemEntity {
    /// Creates one new Pending plan item.
    pub fn new(
        id: impl Into<String>,
        goal: impl AsRef<str>,
    ) -> Result<Self, AssistantProductionPlanError> {
        Ok(Self {
            id: AssistantPlanItemId::new(id)?,
            goal: AssistantPlanItemGoal::new(goal)?,
            state: AssistantPlanItemState::Pending,
        })
    }

    /// Restores one persisted item while revalidating its identity and goal.
    pub fn try_restore(
        id: impl Into<String>,
        goal: impl AsRef<str>,
        state: AssistantPlanItemState,
    ) -> Result<Self, AssistantProductionPlanError> {
        Ok(Self {
            id: AssistantPlanItemId::new(id)?,
            goal: AssistantPlanItemGoal::new(goal)?,
            state,
        })
    }

    /// Returns the stable item identity.
    #[must_use]
    pub fn id(&self) -> &AssistantPlanItemId {
        &self.id
    }

    /// Returns the user-meaningful goal.
    #[must_use]
    pub fn goal(&self) -> &AssistantPlanItemGoal {
        &self.goal
    }

    /// Returns the current aggregate-owned state.
    #[must_use]
    pub const fn state(&self) -> &AssistantPlanItemState {
        &self.state
    }
}

/// Non-zero optimistic-concurrency revision of a production plan.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssistantProductionPlanRevision(u64);

impl AssistantProductionPlanRevision {
    /// Returns the initial revision.
    #[must_use]
    pub const fn initial() -> Self {
        Self(1)
    }

    /// Restores a persisted non-zero revision.
    pub fn new(value: u64) -> Result<Self, AssistantProductionPlanError> {
        if value == 0 {
            Err(AssistantProductionPlanError::InvalidRevision)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the stored non-zero revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Durable Assistant working memory; never an execution queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantProductionPlanAggregate {
    id: AssistantProductionPlanId,
    project_id: ProjectId,
    session_id: AssistantSessionId,
    title: AssistantPlanTitle,
    items: Vec<AssistantPlanItemEntity>,
    revision: AssistantProductionPlanRevision,
}

impl AssistantProductionPlanAggregate {
    /// Creates one initial production plan.
    pub fn new(
        id: AssistantProductionPlanId,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        title: impl AsRef<str>,
        items: Vec<AssistantPlanItemEntity>,
    ) -> Result<Self, AssistantProductionPlanError> {
        validate_items(&items)?;
        Ok(Self {
            id,
            project_id,
            session_id,
            title: AssistantPlanTitle::new(title)?,
            items,
            revision: AssistantProductionPlanRevision::initial(),
        })
    }

    /// Restores one persisted plan while rechecking all aggregate invariants.
    pub fn try_restore(
        id: AssistantProductionPlanId,
        project_id: ProjectId,
        session_id: AssistantSessionId,
        title: impl AsRef<str>,
        items: Vec<AssistantPlanItemEntity>,
        revision: u64,
    ) -> Result<Self, AssistantProductionPlanError> {
        validate_items(&items)?;
        Ok(Self {
            id,
            project_id,
            session_id,
            title: AssistantPlanTitle::new(title)?,
            items,
            revision: AssistantProductionPlanRevision::new(revision)?,
        })
    }

    /// Returns the plan identity.
    #[must_use]
    pub const fn id(&self) -> AssistantProductionPlanId {
        self.id
    }

    /// Returns the owning Project.
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Returns the owning Assistant Session.
    #[must_use]
    pub const fn session_id(&self) -> AssistantSessionId {
        self.session_id
    }

    /// Returns the bounded title.
    #[must_use]
    pub const fn title(&self) -> &AssistantPlanTitle {
        &self.title
    }

    /// Returns the current revision.
    #[must_use]
    pub const fn revision(&self) -> AssistantProductionPlanRevision {
        self.revision
    }

    /// Returns plan items in their persisted presentation order.
    #[must_use]
    pub fn items(&self) -> &[AssistantPlanItemEntity] {
        &self.items
    }

    /// Replaces editable working memory under optimistic revision control.
    pub fn replace(
        &mut self,
        expected_revision: u64,
        title: impl AsRef<str>,
        items: Vec<AssistantPlanItemEntity>,
    ) -> Result<(), AssistantProductionPlanError> {
        self.check_revision(expected_revision)?;
        let title = AssistantPlanTitle::new(title)?;
        validate_items(&items)?;
        let revision =
            self.revision.0.checked_add(1).ok_or(AssistantProductionPlanError::RevisionOverflow)?;
        self.title = title;
        self.items = items;
        self.revision = AssistantProductionPlanRevision(revision);
        Ok(())
    }

    /// Moves a Pending or Blocked item into progress.
    pub fn start_item(
        &mut self,
        expected_revision: u64,
        item_id: &AssistantPlanItemId,
    ) -> Result<(), AssistantProductionPlanError> {
        self.transition_item(expected_revision, item_id, |state| {
            matches!(
                state,
                AssistantPlanItemState::Pending | AssistantPlanItemState::Blocked { .. }
            )
            .then_some(AssistantPlanItemState::InProgress)
        })
    }

    /// Blocks a Pending or InProgress item with one reason.
    pub fn block_item(
        &mut self,
        expected_revision: u64,
        item_id: &AssistantPlanItemId,
        reason: impl AsRef<str>,
    ) -> Result<(), AssistantProductionPlanError> {
        let reason = AssistantPlanItemBlockedReason::new(reason)?;
        self.transition_item(expected_revision, item_id, move |state| {
            matches!(state, AssistantPlanItemState::Pending | AssistantPlanItemState::InProgress)
                .then_some(AssistantPlanItemState::Blocked { reason })
        })
    }

    /// Completes an InProgress item with one acceptance note.
    pub fn complete_item(
        &mut self,
        expected_revision: u64,
        item_id: &AssistantPlanItemId,
        acceptance_note: impl AsRef<str>,
    ) -> Result<(), AssistantProductionPlanError> {
        let acceptance_note = AssistantPlanItemAcceptanceNote::new(acceptance_note)?;
        self.transition_item(expected_revision, item_id, move |state| {
            matches!(state, AssistantPlanItemState::InProgress)
                .then_some(AssistantPlanItemState::Completed { acceptance_note })
        })
    }

    fn transition_item(
        &mut self,
        expected_revision: u64,
        item_id: &AssistantPlanItemId,
        transition: impl FnOnce(&AssistantPlanItemState) -> Option<AssistantPlanItemState>,
    ) -> Result<(), AssistantProductionPlanError> {
        self.check_revision(expected_revision)?;
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id() == item_id)
            .ok_or(AssistantProductionPlanError::ItemNotFound)?;
        item.state =
            transition(&item.state).ok_or(AssistantProductionPlanError::InvalidItemTransition)?;
        self.revision = AssistantProductionPlanRevision(
            self.revision.0.checked_add(1).ok_or(AssistantProductionPlanError::RevisionOverflow)?,
        );
        Ok(())
    }

    fn check_revision(&self, expected: u64) -> Result<(), AssistantProductionPlanError> {
        let actual = self.revision.get();
        if expected == actual {
            Ok(())
        } else {
            Err(AssistantProductionPlanError::RevisionConflict { expected, actual })
        }
    }
}

fn validate_items(items: &[AssistantPlanItemEntity]) -> Result<(), AssistantProductionPlanError> {
    if items.len() > 128 {
        return Err(AssistantProductionPlanError::TooManyItems);
    }
    let unique_ids = items.iter().map(AssistantPlanItemEntity::id).collect::<BTreeSet<_>>();
    if unique_ids.len() == items.len() {
        Ok(())
    } else {
        Err(AssistantProductionPlanError::DuplicateItemId)
    }
}
