// Author: Jeff
// Date: 2026-09-01
// Description: Durable commitment and verification kernel for mg-plan
// Notes: SQLite persistence is local-first; cross-application transport is deferred

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

const MIN_NONEMPTY_TEXT: &str = "value must not be empty";

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(String);

        impl $name {
            // Construct a validated opaque identifier
            pub fn new(value: impl Into<String>) -> Result<Self, PlanError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(PlanError::EmptyIdentifier);
                }
                Ok(Self(value))
            }

            // Borrow the identifier without exposing its representation
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            // Render the opaque identifier
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

define_id!(PlanId);
define_id!(WorkItemId);
define_id!(CriterionId);
define_id!(VerificationId);
define_id!(EvidenceId);
define_id!(ProjectId);
define_id!(MilestoneId);
define_id!(DecisionId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkItemStatus {
    Planned,
    InProgress,
    Blocked,
    Completed,
}

impl fmt::Display for WorkItemStatus {
    // Render a status for transition errors
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = match self {
            Self::Planned => "planned",
            Self::InProgress => "in-progress",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
        };
        formatter.write_str(status)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VerificationResult {
    Pass,
    Fail,
    Inconclusive,
    Waived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceRef {
    id: EvidenceId,
    producer: String,
    source_record: String,
    revision: String,
    digest: String,
}

impl EvidenceRef {
    // Construct a reference to producer-owned evidence
    pub fn new(
        id: EvidenceId,
        producer: impl Into<String>,
        source_record: impl Into<String>,
        revision: impl Into<String>,
        digest: impl Into<String>,
    ) -> Result<Self, PlanError> {
        let producer = producer.into();
        let source_record = source_record.into();
        let revision = revision.into();
        let digest = digest.into();
        if producer.trim().is_empty()
            || source_record.trim().is_empty()
            || revision.trim().is_empty()
            || digest.trim().is_empty()
        {
            return Err(PlanError::EmptyEvidenceReference);
        }
        Ok(Self {
            id,
            producer,
            source_record,
            revision,
            digest,
        })
    }

    // Return the stable evidence identifier
    pub fn id(&self) -> &EvidenceId {
        &self.id
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Verification {
    pub id: VerificationId,
    pub criterion_id: CriterionId,
    pub subject_revision: u64,
    pub evidence: Vec<EvidenceRef>,
    pub result: VerificationResult,
    pub verifier: String,
    pub attempt: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationInput {
    pub id: VerificationId,
    pub criterion_id: CriterionId,
    pub subject_revision: u64,
    pub evidence: Vec<EvidenceRef>,
    pub result: VerificationResult,
    pub verifier: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AcceptanceCriterion {
    pub id: CriterionId,
    pub statement: String,
    pub verifications: Vec<Verification>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub title: String,
    pub status: WorkItemStatus,
    pub revision: u64,
    pub criteria: Vec<AcceptanceCriterion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkItemSummary {
    pub id: WorkItemId,
    pub title: String,
    pub status: WorkItemStatus,
    pub revision: u64,
    pub criteria_count: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum VerificationGapReason {
    MissingVerification,
    NonPassingVerification,
    StaleVerification,
    MissingEvidence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationGap {
    pub work_item_id: WorkItemId,
    pub criterion_id: CriterionId,
    pub reason: VerificationGapReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub milestone_ids: Vec<MilestoneId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Milestone {
    pub id: MilestoneId,
    pub project_id: ProjectId,
    pub title: String,
    pub work_item_ids: BTreeSet<WorkItemId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Decision {
    pub id: DecisionId,
    pub question: String,
    pub decision: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MilestoneSummary {
    pub id: MilestoneId,
    pub project_id: ProjectId,
    pub title: String,
    pub total_work_items: usize,
    pub completed_work_items: usize,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Plan {
    id: PlanId,
    title: String,
    work_items: BTreeMap<WorkItemId, WorkItem>,
    dependencies: BTreeSet<(WorkItemId, WorkItemId)>,
    #[serde(default)]
    projects: BTreeMap<ProjectId, Project>,
    #[serde(default)]
    milestones: BTreeMap<MilestoneId, Milestone>,
    #[serde(default)]
    decisions: BTreeMap<DecisionId, Decision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanError {
    EmptyIdentifier,
    EmptyText(&'static str),
    EmptyEvidenceReference,
    DuplicateWorkItem(WorkItemId),
    DuplicateVerification(VerificationId),
    WorkItemNotFound(WorkItemId),
    CriterionNotFound(CriterionId),
    DuplicateCriterion(CriterionId),
    DuplicateDependency,
    SelfDependency,
    DependencyCycle,
    NoCriteria(WorkItemId),
    DependencyIncomplete(WorkItemId),
    CriterionIncomplete(CriterionId),
    PassRequiresEvidence,
    StaleVerification {
        expected: u64,
        actual: u64,
    },
    RevisionOverflow,
    AttemptOverflow,
    InvalidTransition {
        from: WorkItemStatus,
        to: WorkItemStatus,
    },
    EmptyVerifier,
    DuplicateProject(ProjectId),
    DuplicateMilestone(MilestoneId),
    DuplicateDecision(DecisionId),
    ProjectNotFound(ProjectId),
    MilestoneNotFound(MilestoneId),
    WorkItemAlreadyLinked(WorkItemId),
}

impl fmt::Display for PlanError {
    // Explain a domain rejection without leaking implementation details
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => formatter.write_str("identifier must not be empty"),
            Self::EmptyText(field) => write!(formatter, "{field}: {MIN_NONEMPTY_TEXT}"),
            Self::EmptyEvidenceReference => formatter.write_str("evidence reference is incomplete"),
            Self::DuplicateWorkItem(id) => write!(formatter, "work item already exists: {id}"),
            Self::DuplicateVerification(id) => {
                write!(formatter, "verification already exists: {id}")
            }
            Self::WorkItemNotFound(id) => write!(formatter, "work item not found: {id}"),
            Self::CriterionNotFound(id) => write!(formatter, "criterion not found: {id}"),
            Self::DuplicateCriterion(id) => write!(formatter, "criterion already exists: {id}"),
            Self::DuplicateDependency => formatter.write_str("dependency already exists"),
            Self::SelfDependency => formatter.write_str("work item cannot depend on itself"),
            Self::DependencyCycle => formatter.write_str("dependency would create a cycle"),
            Self::NoCriteria(id) => write!(formatter, "work item has no acceptance criteria: {id}"),
            Self::DependencyIncomplete(id) => write!(formatter, "dependency is incomplete: {id}"),
            Self::CriterionIncomplete(id) => write!(formatter, "criterion is not verified: {id}"),
            Self::PassRequiresEvidence => {
                formatter.write_str("a passing verification requires evidence")
            }
            Self::StaleVerification { expected, actual } => {
                write!(
                    formatter,
                    "verification targets revision {actual}, expected {expected}"
                )
            }
            Self::RevisionOverflow => formatter.write_str("work item revision overflow"),
            Self::AttemptOverflow => formatter.write_str("verification attempt overflow"),
            Self::InvalidTransition { from, to } => {
                write!(formatter, "invalid work-item transition: {from} -> {to}")
            }
            Self::EmptyVerifier => formatter.write_str("verifier must not be empty"),
            Self::DuplicateProject(id) => write!(formatter, "project already exists: {id}"),
            Self::DuplicateMilestone(id) => write!(formatter, "milestone already exists: {id}"),
            Self::DuplicateDecision(id) => write!(formatter, "decision already exists: {id}"),
            Self::ProjectNotFound(id) => write!(formatter, "project not found: {id}"),
            Self::MilestoneNotFound(id) => write!(formatter, "milestone not found: {id}"),
            Self::WorkItemAlreadyLinked(id) => write!(formatter, "work item already linked: {id}"),
        }
    }
}

impl std::error::Error for PlanError {}

impl Plan {
    // Create an empty plan with no executable work yet
    pub fn new(id: PlanId, title: impl Into<String>) -> Result<Self, PlanError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(PlanError::EmptyText("plan title"));
        }
        Ok(Self {
            id,
            title,
            work_items: BTreeMap::new(),
            dependencies: BTreeSet::new(),
            projects: BTreeMap::new(),
            milestones: BTreeMap::new(),
            decisions: BTreeMap::new(),
        })
    }

    // Return the stable plan identifier
    pub fn id(&self) -> &PlanId {
        &self.id
    }

    // Return the validated plan title
    pub fn title(&self) -> &str {
        &self.title
    }

    // Return deterministic compact summaries for projection clients
    pub fn work_item_summaries(&self) -> Vec<WorkItemSummary> {
        self.work_items
            .values()
            .map(|item| WorkItemSummary {
                id: item.id.clone(),
                title: item.title.clone(),
                status: item.status,
                revision: item.revision,
                criteria_count: item.criteria.len(),
            })
            .collect()
    }

    // Return only explicitly blocked work in stable identifier order
    pub fn blocked_work_item_summaries(&self) -> Vec<WorkItemSummary> {
        self.work_item_summaries()
            .into_iter()
            .filter(|item| item.status == WorkItemStatus::Blocked)
            .collect()
    }

    // Explain every criterion that cannot currently prove completion
    pub fn verification_gaps(&self) -> Vec<VerificationGap> {
        self.work_items
            .values()
            .flat_map(|item| {
                item.criteria.iter().filter_map(|criterion| {
                    let reason = match criterion.verifications.last() {
                        None => VerificationGapReason::MissingVerification,
                        Some(verification) if verification.evidence.is_empty() => {
                            VerificationGapReason::MissingEvidence
                        }
                        Some(verification) if verification.subject_revision != item.revision => {
                            VerificationGapReason::StaleVerification
                        }
                        Some(verification) if verification.result != VerificationResult::Pass => {
                            VerificationGapReason::NonPassingVerification
                        }
                        Some(_) => return None,
                    };
                    Some(VerificationGap {
                        work_item_id: item.id.clone(),
                        criterion_id: criterion.id.clone(),
                        reason,
                    })
                })
            })
            .collect()
    }

    // Create a project container without duplicating work-item authority
    pub fn add_project(
        &mut self,
        id: ProjectId,
        title: impl Into<String>,
    ) -> Result<(), PlanError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(PlanError::EmptyText("project title"));
        }
        if self.projects.contains_key(&id) {
            return Err(PlanError::DuplicateProject(id));
        }
        self.projects.insert(
            id.clone(),
            Project {
                id,
                title,
                milestone_ids: Vec::new(),
            },
        );
        Ok(())
    }

    // Create a milestone owned by an existing project
    pub fn add_milestone(
        &mut self,
        project_id: &ProjectId,
        id: MilestoneId,
        title: impl Into<String>,
    ) -> Result<(), PlanError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(PlanError::EmptyText("milestone title"));
        }
        if !self.projects.contains_key(project_id) {
            return Err(PlanError::ProjectNotFound(project_id.clone()));
        }
        if self.milestones.contains_key(&id) {
            return Err(PlanError::DuplicateMilestone(id));
        }
        self.milestones.insert(
            id.clone(),
            Milestone {
                id: id.clone(),
                project_id: project_id.clone(),
                title,
                work_item_ids: BTreeSet::new(),
            },
        );
        self.projects
            .get_mut(project_id)
            .expect("project was validated above")
            .milestone_ids
            .push(id);
        Ok(())
    }

    // Link existing work to a milestone while invalidating its proof revision
    pub fn link_work_item_to_milestone(
        &mut self,
        milestone_id: &MilestoneId,
        work_item_id: &WorkItemId,
    ) -> Result<(), PlanError> {
        self.work_item(work_item_id)?;
        let milestone = self
            .milestones
            .get(milestone_id)
            .ok_or_else(|| PlanError::MilestoneNotFound(milestone_id.clone()))?;
        if milestone.work_item_ids.contains(work_item_id) {
            return Err(PlanError::WorkItemAlreadyLinked(work_item_id.clone()));
        }
        let impacted = self.impacted_items(work_item_id);
        self.ensure_revision_capacity(&impacted)?;
        self.milestones
            .get_mut(milestone_id)
            .expect("milestone was validated above")
            .work_item_ids
            .insert(work_item_id.clone());
        self.bump_revisions(&impacted)?;
        Ok(())
    }

    // Record an immutable decision inside the plan aggregate
    pub fn record_decision(
        &mut self,
        id: DecisionId,
        question: impl Into<String>,
        decision: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Result<(), PlanError> {
        let question = question.into();
        let decision = decision.into();
        let rationale = rationale.into();
        if question.trim().is_empty() {
            return Err(PlanError::EmptyText("decision question"));
        }
        if decision.trim().is_empty() {
            return Err(PlanError::EmptyText("decision"));
        }
        if rationale.trim().is_empty() {
            return Err(PlanError::EmptyText("decision rationale"));
        }
        if self.decisions.contains_key(&id) {
            return Err(PlanError::DuplicateDecision(id));
        }
        self.decisions.insert(
            id.clone(),
            Decision {
                id,
                question,
                decision,
                rationale,
            },
        );
        Ok(())
    }

    // Derive milestone progress from the authoritative work-item statuses
    pub fn milestone_summaries(&self) -> Vec<MilestoneSummary> {
        self.milestones
            .values()
            .map(|milestone| {
                let total_work_items = milestone.work_item_ids.len();
                let completed_work_items = milestone
                    .work_item_ids
                    .iter()
                    .filter(|id| {
                        self.work_item(id)
                            .is_ok_and(|item| item.status == WorkItemStatus::Completed)
                    })
                    .count();
                MilestoneSummary {
                    id: milestone.id.clone(),
                    project_id: milestone.project_id.clone(),
                    title: milestone.title.clone(),
                    total_work_items,
                    completed_work_items,
                    complete: total_work_items > 0 && completed_work_items == total_work_items,
                }
            })
            .collect()
    }

    // Add a planned work item
    pub fn add_work_item(
        &mut self,
        id: WorkItemId,
        title: impl Into<String>,
    ) -> Result<(), PlanError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(PlanError::EmptyText("work item title"));
        }
        if self.work_items.contains_key(&id) {
            return Err(PlanError::DuplicateWorkItem(id));
        }
        self.work_items.insert(
            id.clone(),
            WorkItem {
                id,
                title,
                status: WorkItemStatus::Planned,
                revision: 1,
                criteria: Vec::new(),
            },
        );
        Ok(())
    }

    // Return a work item by stable identifier
    pub fn work_item(&self, id: &WorkItemId) -> Result<&WorkItem, PlanError> {
        self.work_items
            .get(id)
            .ok_or_else(|| PlanError::WorkItemNotFound(id.clone()))
    }

    // Start a work item after its identity has been established
    pub fn start_work(&mut self, id: &WorkItemId) -> Result<(), PlanError> {
        let work_item = self.work_item_mut(id)?;
        match work_item.status {
            WorkItemStatus::Planned | WorkItemStatus::InProgress => {
                work_item.status = WorkItemStatus::InProgress;
            }
            status => {
                return Err(PlanError::InvalidTransition {
                    from: status,
                    to: WorkItemStatus::InProgress,
                });
            }
        }
        Ok(())
    }

    // Mark active work as blocked by an explicit obstacle
    pub fn block_work(&mut self, id: &WorkItemId) -> Result<(), PlanError> {
        let work_item = self.work_item_mut(id)?;
        match work_item.status {
            WorkItemStatus::Planned | WorkItemStatus::InProgress => {
                work_item.status = WorkItemStatus::Blocked;
                Ok(())
            }
            status => Err(PlanError::InvalidTransition {
                from: status,
                to: WorkItemStatus::Blocked,
            }),
        }
    }

    // Return blocked work to the planned state after its obstacle is resolved
    pub fn unblock_work(&mut self, id: &WorkItemId) -> Result<(), PlanError> {
        let work_item = self.work_item_mut(id)?;
        if work_item.status != WorkItemStatus::Blocked {
            return Err(PlanError::InvalidTransition {
                from: work_item.status,
                to: WorkItemStatus::Planned,
            });
        }
        work_item.status = WorkItemStatus::Planned;
        Ok(())
    }

    // Add a prerequisite edge while rejecting cycles
    pub fn add_dependency(
        &mut self,
        work_item: &WorkItemId,
        prerequisite: &WorkItemId,
    ) -> Result<(), PlanError> {
        self.work_item(work_item)?;
        self.work_item(prerequisite)?;
        if work_item == prerequisite {
            return Err(PlanError::SelfDependency);
        }
        if self
            .dependencies
            .contains(&(work_item.clone(), prerequisite.clone()))
        {
            return Err(PlanError::DuplicateDependency);
        }
        if self.reachable(prerequisite, work_item) {
            return Err(PlanError::DependencyCycle);
        }
        let impacted = self.impacted_items(work_item);
        self.ensure_revision_capacity(&impacted)?;
        self.dependencies
            .insert((work_item.clone(), prerequisite.clone()));
        self.bump_revisions(&impacted)?;
        Ok(())
    }

    // Add an acceptance criterion to a work item
    pub fn add_criterion(
        &mut self,
        work_item: &WorkItemId,
        id: CriterionId,
        statement: impl Into<String>,
    ) -> Result<(), PlanError> {
        let statement = statement.into();
        if statement.trim().is_empty() {
            return Err(PlanError::EmptyText("criterion statement"));
        }
        let impacted = self.impacted_items(work_item);
        self.ensure_revision_capacity(&impacted)?;
        let item = self.work_item_mut(work_item)?;
        if item.criteria.iter().any(|criterion| criterion.id == id) {
            return Err(PlanError::DuplicateCriterion(id));
        }
        item.criteria.push(AcceptanceCriterion {
            id,
            statement,
            verifications: Vec::new(),
        });
        self.bump_revisions(&impacted)?;
        Ok(())
    }

    // Record a revision-pinned verification attempt
    pub fn record_verification(
        &mut self,
        work_item: &WorkItemId,
        input: VerificationInput,
    ) -> Result<(), PlanError> {
        let VerificationInput {
            id,
            criterion_id,
            subject_revision,
            evidence,
            result,
            verifier,
        } = input;
        if verifier.trim().is_empty() {
            return Err(PlanError::EmptyVerifier);
        }
        if result == VerificationResult::Pass && evidence.is_empty() {
            return Err(PlanError::PassRequiresEvidence);
        }
        if self.work_items.values().any(|item| {
            item.criteria.iter().any(|criterion| {
                criterion
                    .verifications
                    .iter()
                    .any(|verification| verification.id == id)
            })
        }) {
            return Err(PlanError::DuplicateVerification(id));
        }
        let item = self.work_item(work_item)?.clone();
        if item.revision != subject_revision {
            return Err(PlanError::StaleVerification {
                expected: item.revision,
                actual: subject_revision,
            });
        }
        let criterion_index = item
            .criteria
            .iter()
            .position(|criterion| criterion.id == criterion_id)
            .ok_or_else(|| PlanError::CriterionNotFound(criterion_id.clone()))?;
        let completed_dependents = if item.status == WorkItemStatus::Completed {
            self.impacted_items(work_item)
                .into_iter()
                .filter(|id| id != work_item)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        self.ensure_revision_capacity(&completed_dependents)?;
        let work_item = self.work_item_mut(work_item)?;
        let criterion = work_item
            .criteria
            .get_mut(criterion_index)
            .ok_or_else(|| PlanError::CriterionNotFound(criterion_id.clone()))?;
        let attempt = (criterion.verifications.len() as u64)
            .checked_add(1)
            .ok_or(PlanError::AttemptOverflow)?;
        criterion.verifications.push(Verification {
            id,
            criterion_id: criterion_id.clone(),
            subject_revision,
            evidence,
            result,
            verifier,
            attempt,
        });
        if work_item.status == WorkItemStatus::Completed {
            work_item.status = WorkItemStatus::Planned;
        }
        self.bump_revisions(&completed_dependents)?;
        Ok(())
    }

    // Complete a work item only when prerequisites and criteria are proven
    pub fn complete(&mut self, id: &WorkItemId) -> Result<(), PlanError> {
        let current_status = self.work_item(id)?.status;
        if current_status == WorkItemStatus::Completed {
            return Err(PlanError::InvalidTransition {
                from: current_status,
                to: WorkItemStatus::Completed,
            });
        }
        if current_status == WorkItemStatus::Blocked {
            return Err(PlanError::InvalidTransition {
                from: current_status,
                to: WorkItemStatus::Completed,
            });
        }
        let prerequisites: Vec<WorkItemId> = self
            .dependencies
            .iter()
            .filter(|(dependent, _)| dependent == id)
            .map(|(_, prerequisite)| prerequisite.clone())
            .collect();
        for prerequisite in prerequisites {
            if self.work_item(&prerequisite)?.status != WorkItemStatus::Completed {
                return Err(PlanError::DependencyIncomplete(prerequisite));
            }
        }
        let item = self.work_item(id)?;
        if item.criteria.is_empty() {
            return Err(PlanError::NoCriteria(id.clone()));
        }
        for criterion in &item.criteria {
            let verified = criterion.verifications.last().is_some_and(|verification| {
                verification.result == VerificationResult::Pass
                    && verification.subject_revision == item.revision
                    && !verification.evidence.is_empty()
            });
            if !verified {
                return Err(PlanError::CriterionIncomplete(criterion.id.clone()));
            }
        }
        self.work_item_mut(id)?.status = WorkItemStatus::Completed;
        Ok(())
    }

    // Revise a work item and invalidate prior revision-pinned completion proof
    pub fn revise_work_item(
        &mut self,
        id: &WorkItemId,
        title: impl Into<String>,
    ) -> Result<(), PlanError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(PlanError::EmptyText("work item title"));
        }
        let impacted = self.impacted_items(id);
        self.ensure_revision_capacity(&impacted)?;
        let previous_status = self.work_item(id)?.status;
        self.bump_revisions(&impacted)?;
        let item = self.work_item_mut(id)?;
        item.title = title;
        item.status = if previous_status == WorkItemStatus::Completed {
            WorkItemStatus::Planned
        } else {
            previous_status
        };
        Ok(())
    }

    // Borrow the mutable work item after centralizing not-found behavior
    fn work_item_mut(&mut self, id: &WorkItemId) -> Result<&mut WorkItem, PlanError> {
        self.work_items
            .get_mut(id)
            .ok_or_else(|| PlanError::WorkItemNotFound(id.clone()))
    }

    // Collect a work item and every transitive dependent
    fn impacted_items(&self, root: &WorkItemId) -> Vec<WorkItemId> {
        let mut pending = vec![root.clone()];
        let mut impacted = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !impacted.insert(current.clone()) {
                continue;
            }
            pending.extend(
                self.dependencies
                    .iter()
                    .filter(|(_, prerequisite)| prerequisite == &current)
                    .map(|(dependent, _)| dependent.clone()),
            );
        }
        impacted.into_iter().collect()
    }

    // Reject a revision cascade that cannot be represented safely
    fn ensure_revision_capacity(&self, ids: &[WorkItemId]) -> Result<(), PlanError> {
        for id in ids {
            self.work_item(id)?
                .revision
                .checked_add(1)
                .ok_or(PlanError::RevisionOverflow)?;
        }
        Ok(())
    }

    // Advance impacted revisions and invalidate completed proof
    fn bump_revisions(&mut self, ids: &[WorkItemId]) -> Result<(), PlanError> {
        for id in ids {
            let item = self.work_item_mut(id)?;
            item.revision = item
                .revision
                .checked_add(1)
                .ok_or(PlanError::RevisionOverflow)?;
            if item.status == WorkItemStatus::Completed {
                item.status = WorkItemStatus::Planned;
            }
        }
        Ok(())
    }

    // Test whether a dependency path already reaches the target
    fn reachable(&self, start: &WorkItemId, target: &WorkItemId) -> bool {
        let mut pending = vec![start.clone()];
        let mut visited = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if current == *target {
                return true;
            }
            if !visited.insert(current.clone()) {
                continue;
            }
            pending.extend(
                self.dependencies
                    .iter()
                    .filter(|(dependent, _)| dependent == &current)
                    .map(|(_, prerequisite)| prerequisite.clone()),
            );
        }
        false
    }
}

#[derive(Debug)]
pub struct PlanStore {
    connection: Connection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredPlan {
    pub plan: Plan,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationRecord {
    pub revision: u64,
    pub document_json: String,
}

#[derive(Debug)]
pub enum StoreError {
    Sql(rusqlite::Error),
    Json(serde_json::Error),
    InvalidStoredPlan,
    PlanNotFound(PlanId),
    RevisionConflict {
        id: PlanId,
        expected: u64,
        actual: u64,
    },
    RevisionOverflow,
}

impl fmt::Display for StoreError {
    // Explain storage failures without exposing database internals
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(_) => formatter.write_str("plan storage operation failed"),
            Self::Json(_) => formatter.write_str("plan document is invalid"),
            Self::InvalidStoredPlan => formatter.write_str("stored plan identity is invalid"),
            Self::PlanNotFound(id) => write!(formatter, "plan not found: {id}"),
            Self::RevisionConflict {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "revision conflict for {id}: expected {expected}, actual {actual}"
            ),
            Self::RevisionOverflow => formatter.write_str("plan revision overflow"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    // Convert database errors to the storage error boundary
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

impl From<serde_json::Error> for StoreError {
    // Convert serialization errors to the storage error boundary
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl PlanStore {
    // Open a SQLite plan store and apply its migrations
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        let mut store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    // Open an isolated store for tests and embedders
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        let mut store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    // Apply the initial schema transactionally and idempotently
    fn migrate(&mut self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations (
                 version INTEGER PRIMARY KEY
             );
             CREATE TABLE IF NOT EXISTS plans (
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 document_json TEXT NOT NULL,
                 revision INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE IF NOT EXISTS mutation_history (
                 plan_id TEXT NOT NULL,
                 revision INTEGER NOT NULL,
                 document_json TEXT NOT NULL,
                 PRIMARY KEY (plan_id, revision)
             );
             INSERT OR IGNORE INTO schema_migrations (version) VALUES (1);",
        )?;
        let has_revision = self
            .connection
            .prepare("PRAGMA table_info(plans)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|name| name == "revision");
        if !has_revision {
            self.connection.execute(
                "ALTER TABLE plans ADD COLUMN revision INTEGER NOT NULL DEFAULT 1",
                [],
            )?;
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO mutation_history (plan_id, revision, document_json)
             SELECT id, revision, document_json FROM plans",
            [],
        )?;
        self.connection.execute(
            "INSERT OR IGNORE INTO schema_migrations (version) VALUES (2)",
            [],
        )?;
        Ok(())
    }

    // Create a plan and its initial immutable history record
    pub fn create(&mut self, plan: &Plan) -> Result<StoredPlan, StoreError> {
        let document = serde_json::to_string(plan)?;
        let transaction = self.connection.transaction()?;
        let inserted = transaction.execute(
            "INSERT INTO plans (id, title, document_json, revision) VALUES (?1, ?2, ?3, 1)",
            params![plan.id.as_str(), plan.title, document],
        );
        if let Err(rusqlite::Error::SqliteFailure(error, _)) = inserted {
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY {
                return Err(StoreError::RevisionConflict {
                    id: plan.id.clone(),
                    expected: 0,
                    actual: 1,
                });
            }
            inserted?;
        } else {
            inserted?;
        }
        transaction.execute(
            "INSERT INTO mutation_history (plan_id, revision, document_json) VALUES (?1, 1, ?2)",
            params![plan.id.as_str(), document],
        )?;
        transaction.commit()?;
        Ok(StoredPlan {
            plan: plan.clone(),
            revision: 1,
        })
    }

    // Compatibility save that serializes against the current database revision
    pub fn save(&mut self, plan: &Plan) -> Result<(), StoreError> {
        match self.load_versioned(plan.id()) {
            Ok(stored) => {
                self.save_if_revision(plan, stored.revision)?;
            }
            Err(StoreError::PlanNotFound(_)) => {
                self.create(plan)?;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    // Replace a plan only if the caller still owns the observed revision
    pub fn save_if_revision(
        &mut self,
        plan: &Plan,
        expected_revision: u64,
    ) -> Result<StoredPlan, StoreError> {
        let document = serde_json::to_string(plan)?;
        let transaction = self.connection.transaction()?;
        let actual: u64 = transaction
            .query_row(
                "SELECT revision FROM plans WHERE id = ?1",
                params![plan.id.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StoreError::PlanNotFound(plan.id.clone()),
                other => StoreError::Sql(other),
            })?;
        if actual != expected_revision {
            return Err(StoreError::RevisionConflict {
                id: plan.id.clone(),
                expected: expected_revision,
                actual,
            });
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(StoreError::RevisionOverflow)?;
        transaction.execute(
            "UPDATE plans SET title = ?1, document_json = ?2, revision = ?3
             WHERE id = ?4 AND revision = ?5",
            params![
                plan.title,
                document,
                revision,
                plan.id.as_str(),
                expected_revision
            ],
        )?;
        transaction.execute(
            "INSERT INTO mutation_history (plan_id, revision, document_json) VALUES (?1, ?2, ?3)",
            params![plan.id.as_str(), revision, document],
        )?;
        transaction.commit()?;
        Ok(StoredPlan {
            plan: plan.clone(),
            revision,
        })
    }

    // Load and validate one complete plan aggregate with its store revision
    pub fn load_versioned(&self, id: &PlanId) -> Result<StoredPlan, StoreError> {
        let (document, revision): (String, u64) = self
            .connection
            .query_row(
                "SELECT document_json, revision FROM plans WHERE id = ?1",
                params![id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => StoreError::PlanNotFound(id.clone()),
                other => StoreError::Sql(other),
            })?;
        let plan: Plan = serde_json::from_str(&document)?;
        if plan.id() != id {
            return Err(StoreError::InvalidStoredPlan);
        }
        Ok(StoredPlan { plan, revision })
    }

    // Load one complete plan without exposing its persistence revision
    pub fn load(&self, id: &PlanId) -> Result<Plan, StoreError> {
        Ok(self.load_versioned(id)?.plan)
    }

    // Read the append-only aggregate history for audit and recovery
    pub fn history(&self, id: &PlanId) -> Result<Vec<MutationRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT revision, document_json FROM mutation_history
             WHERE plan_id = ?1 ORDER BY revision",
        )?;
        let records = statement
            .query_map(params![id.as_str()], |row| {
                Ok(MutationRecord {
                    revision: row.get(0)?,
                    document_json: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if records.is_empty() && self.load_versioned(id).is_err() {
            return Err(StoreError::PlanNotFound(id.clone()));
        }
        Ok(records)
    }

    // Export one plan as a portable versioned-domain document
    pub fn export_json(&self, id: &PlanId) -> Result<String, StoreError> {
        Ok(serde_json::to_string_pretty(&self.load(id)?)?)
    }

    // Validate and atomically import one complete plan document
    pub fn import_json(&mut self, document: &str) -> Result<PlanId, StoreError> {
        let plan: Plan = serde_json::from_str(document)?;
        let id = plan.id().clone();
        self.save(&plan)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<T: FromId>(value: &str) -> T {
        T::from_id(value)
    }

    trait FromId {
        fn from_id(value: &str) -> Self;
    }

    macro_rules! impl_from_id {
        ($($type:ty),+ $(,)?) => {
            $(impl FromId for $type {
                fn from_id(value: &str) -> Self {
                    <$type>::new(value).expect("test IDs are nonempty")
                }
            })+
        };
    }

    impl_from_id!(
        PlanId,
        WorkItemId,
        CriterionId,
        VerificationId,
        EvidenceId,
        ProjectId,
        MilestoneId,
        DecisionId
    );

    fn evidence() -> EvidenceRef {
        EvidenceRef::new(
            id("evidence-1"),
            "mg-lab",
            "run-1",
            "revision-1",
            "sha256:abc",
        )
        .expect("evidence is complete")
    }

    #[test]
    fn completion_requires_revision_pinned_passing_evidence() {
        let mut plan = Plan::new(id("plan-1"), "Ship a verified slice").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        plan.add_work_item(work.clone(), "Implement the slice")
            .expect("work is valid");
        let criterion: CriterionId = id("criterion-1");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");

        assert_eq!(
            plan.complete(&work),
            Err(PlanError::CriterionIncomplete(criterion.clone()))
        );
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: criterion.clone(),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&work).expect("verified work completes");
        assert_eq!(
            plan.work_item(&work).expect("work exists").status,
            WorkItemStatus::Completed
        );
    }

    #[test]
    fn dependencies_reject_cycles_and_incomplete_prerequisites() {
        let mut plan = Plan::new(id("plan-1"), "Dependency test").expect("plan is valid");
        let first: WorkItemId = id("work-1");
        let second: WorkItemId = id("work-2");
        plan.add_work_item(first.clone(), "First")
            .expect("work is valid");
        plan.add_work_item(second.clone(), "Second")
            .expect("work is valid");
        plan.add_dependency(&second, &first).expect("edge is valid");
        assert_eq!(
            plan.add_dependency(&first, &second),
            Err(PlanError::DependencyCycle)
        );
        assert_eq!(
            plan.complete(&second),
            Err(PlanError::DependencyIncomplete(first))
        );
    }

    #[test]
    fn revising_work_invalidates_previous_verification_revision() {
        let mut plan = Plan::new(id("plan-1"), "Revision test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        let criterion: CriterionId = id("criterion-1");
        plan.add_work_item(work.clone(), "Original")
            .expect("work is valid");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: criterion.clone(),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "ci:test".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&work).expect("original work completes");
        plan.revise_work_item(&work, "Revised")
            .expect("revision is valid");
        assert_eq!(plan.work_item(&work).expect("work exists").revision, 3);
        assert_eq!(
            plan.complete(&work),
            Err(PlanError::CriterionIncomplete(criterion))
        );
    }

    #[test]
    fn verification_ids_are_unique_within_a_plan() {
        let mut plan = Plan::new(id("plan-1"), "Identity test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        let criterion: CriterionId = id("criterion-1");
        let verification: VerificationId = id("verification-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");
        let input = || VerificationInput {
            id: verification.clone(),
            criterion_id: criterion.clone(),
            subject_revision: 2,
            evidence: vec![evidence()],
            result: VerificationResult::Pass,
            verifier: "human:jeff".to_owned(),
        };
        plan.record_verification(&work, input())
            .expect("first verification is valid");
        assert_eq!(
            plan.record_verification(&work, input()),
            Err(PlanError::DuplicateVerification(verification))
        );
    }

    #[test]
    fn structural_changes_invalidate_completed_work() {
        let mut plan = Plan::new(id("plan-1"), "Structural mutation test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        let criterion: CriterionId = id("criterion-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: criterion,
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&work).expect("work is complete");
        plan.add_criterion(&work, id("criterion-2"), "A new condition is demonstrated")
            .expect("new criterion is valid");
        assert_eq!(plan.work_item(&work).expect("work exists").revision, 3);
        assert_eq!(
            plan.work_item(&work).expect("work exists").status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            plan.complete(&work),
            Err(PlanError::CriterionIncomplete(id("criterion-1")))
        );
    }

    #[test]
    fn prerequisite_changes_invalidate_completed_dependents() {
        let mut plan =
            Plan::new(id("plan-1"), "Dependency invalidation test").expect("plan is valid");
        let prerequisite: WorkItemId = id("work-1");
        let dependent: WorkItemId = id("work-2");
        let criterion: CriterionId = id("criterion-1");
        plan.add_work_item(prerequisite.clone(), "Prerequisite")
            .expect("work is valid");
        plan.add_work_item(dependent.clone(), "Dependent")
            .expect("work is valid");
        plan.add_dependency(&dependent, &prerequisite)
            .expect("dependency is valid");
        plan.add_criterion(&prerequisite, id("criterion-1"), "Prerequisite is proven")
            .expect("criterion is valid");
        plan.record_verification(
            &prerequisite,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: id("criterion-1"),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&prerequisite)
            .expect("prerequisite is complete");
        plan.add_criterion(&dependent, criterion.clone(), "Dependent is proven")
            .expect("criterion is valid");
        plan.record_verification(
            &dependent,
            VerificationInput {
                id: id("verification-2"),
                criterion_id: criterion,
                subject_revision: 4,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&dependent).expect("dependent is complete");
        plan.revise_work_item(&prerequisite, "Revised prerequisite")
            .expect("revision is valid");
        assert_eq!(
            plan.work_item(&dependent).expect("dependent exists").status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            plan.complete(&dependent),
            Err(PlanError::DependencyIncomplete(prerequisite))
        );
    }

    #[test]
    fn revising_blocked_work_preserves_blocked_state() {
        let mut plan = Plan::new(id("plan-1"), "Blocked revision test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.block_work(&work).expect("work blocks");
        plan.revise_work_item(&work, "Revised work")
            .expect("revision is valid");
        assert_eq!(
            plan.work_item(&work).expect("work exists").status,
            WorkItemStatus::Blocked
        );
    }

    #[test]
    fn blocked_work_cannot_complete_until_unblocked() {
        let mut plan = Plan::new(id("plan-1"), "Transition test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.start_work(&work).expect("work starts");
        plan.block_work(&work).expect("work blocks");
        assert_eq!(
            plan.complete(&work),
            Err(PlanError::InvalidTransition {
                from: WorkItemStatus::Blocked,
                to: WorkItemStatus::Completed,
            })
        );
        plan.unblock_work(&work).expect("work unblocks");
        assert_eq!(plan.start_work(&work), Ok(()));
    }

    #[test]
    fn reopening_completed_prerequisite_reopens_completed_dependents() {
        let mut plan =
            Plan::new(id("plan-1"), "Verification dependency test").expect("plan is valid");
        let prerequisite: WorkItemId = id("work-1");
        let dependent: WorkItemId = id("work-2");
        let prerequisite_criterion: CriterionId = id("criterion-1");
        let dependent_criterion: CriterionId = id("criterion-2");
        plan.add_work_item(prerequisite.clone(), "Prerequisite")
            .expect("work is valid");
        plan.add_work_item(dependent.clone(), "Dependent")
            .expect("work is valid");
        plan.add_dependency(&dependent, &prerequisite)
            .expect("dependency is valid");
        plan.add_criterion(
            &prerequisite,
            prerequisite_criterion.clone(),
            "Prerequisite is proven",
        )
        .expect("criterion is valid");
        plan.record_verification(
            &prerequisite,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: prerequisite_criterion,
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&prerequisite)
            .expect("prerequisite is complete");
        plan.add_criterion(
            &dependent,
            dependent_criterion.clone(),
            "Dependent is proven",
        )
        .expect("criterion is valid");
        plan.record_verification(
            &dependent,
            VerificationInput {
                id: id("verification-2"),
                criterion_id: dependent_criterion.clone(),
                subject_revision: 4,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&dependent).expect("dependent is complete");
        plan.record_verification(
            &prerequisite,
            VerificationInput {
                id: id("verification-3"),
                criterion_id: id("criterion-1"),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Fail,
                verifier: "ci:test".to_owned(),
            },
        )
        .expect("verification is valid");
        assert_eq!(
            plan.work_item(&prerequisite)
                .expect("prerequisite exists")
                .status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            plan.work_item(&dependent).expect("dependent exists").status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            plan.work_item(&dependent)
                .expect("dependent exists")
                .revision,
            5
        );
    }

    #[test]
    fn new_nonpassing_verification_reopens_completed_work() {
        let mut plan = Plan::new(id("plan-1"), "Verification status test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        let criterion: CriterionId = id("criterion-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-1"),
                criterion_id: criterion.clone(),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "human:jeff".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&work).expect("work is complete");
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-2"),
                criterion_id: criterion.clone(),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Fail,
                verifier: "ci:test".to_owned(),
            },
        )
        .expect("verification is valid");
        assert_eq!(
            plan.work_item(&work).expect("work exists").status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            plan.complete(&work),
            Err(PlanError::CriterionIncomplete(criterion))
        );
    }

    #[test]
    fn query_projections_are_deterministic_and_explain_gaps() {
        let mut plan = Plan::new(id("plan-queries"), "Query plan").expect("plan is valid");
        let blocked: WorkItemId = id("blocked");
        let missing: WorkItemId = id("missing");
        let failing: WorkItemId = id("failing");
        let stale: WorkItemId = id("stale");
        for (work, title) in [
            (blocked.clone(), "Blocked"),
            (missing.clone(), "Missing"),
            (failing.clone(), "Failing"),
            (stale.clone(), "Stale"),
        ] {
            plan.add_work_item(work, title).expect("work is valid");
        }
        plan.block_work(&blocked).expect("work blocks");
        plan.add_criterion(&missing, id("missing-criterion"), "Missing proof")
            .expect("criterion is valid");
        let failing_criterion: CriterionId = id("failing-criterion");
        plan.add_criterion(&failing, failing_criterion.clone(), "Failing proof")
            .expect("criterion is valid");
        plan.record_verification(
            &failing,
            VerificationInput {
                id: id("failing-verification"),
                criterion_id: failing_criterion,
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Fail,
                verifier: "test".to_owned(),
            },
        )
        .expect("verification is valid");
        let stale_criterion: CriterionId = id("stale-criterion");
        plan.add_criterion(&stale, stale_criterion.clone(), "Stale proof")
            .expect("criterion is valid");
        plan.record_verification(
            &stale,
            VerificationInput {
                id: id("stale-verification"),
                criterion_id: stale_criterion.clone(),
                subject_revision: 2,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "test".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.revise_work_item(&stale, "Revised stale work")
            .expect("revision is valid");

        let summaries = plan.work_item_summaries();
        assert_eq!(
            summaries
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["blocked", "failing", "missing", "stale"]
        );
        assert_eq!(
            plan.blocked_work_item_summaries()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["blocked"]
        );
        assert_eq!(
            plan.verification_gaps(),
            vec![
                VerificationGap {
                    work_item_id: id("failing"),
                    criterion_id: id("failing-criterion"),
                    reason: VerificationGapReason::NonPassingVerification,
                },
                VerificationGap {
                    work_item_id: id("missing"),
                    criterion_id: id("missing-criterion"),
                    reason: VerificationGapReason::MissingVerification,
                },
                VerificationGap {
                    work_item_id: id("stale"),
                    criterion_id: id("stale-criterion"),
                    reason: VerificationGapReason::StaleVerification,
                },
            ]
        );
    }

    #[test]
    fn project_milestone_decision_slice_is_derived_and_persistent() {
        let mut plan = Plan::new(id("plan-structure"), "Structured plan").expect("plan is valid");
        let work: WorkItemId = id("work-structure");
        plan.add_work_item(work.clone(), "Complete structured work")
            .expect("work is valid");
        let criterion: CriterionId = id("criterion-structure");
        plan.add_criterion(&work, criterion.clone(), "Work is proven")
            .expect("criterion is valid");
        let project: ProjectId = id("project-1");
        let milestone: MilestoneId = id("milestone-1");
        plan.add_project(project.clone(), "Suite foundation")
            .expect("project is valid");
        plan.add_milestone(&project, milestone.clone(), "Verified foundation")
            .expect("milestone is valid");
        plan.link_work_item_to_milestone(&milestone, &work)
            .expect("work links");
        plan.record_decision(
            id("decision-1"),
            "Where does milestone completion come from?",
            "From work-item status",
            "Work items remain the single completion authority",
        )
        .expect("decision is valid");

        let incomplete = plan.milestone_summaries();
        assert_eq!(incomplete[0].total_work_items, 1);
        assert_eq!(incomplete[0].completed_work_items, 0);
        assert!(!incomplete[0].complete);
        let revision = plan.work_item(&work).expect("work exists").revision;
        plan.record_verification(
            &work,
            VerificationInput {
                id: id("verification-structure"),
                criterion_id: criterion,
                subject_revision: revision,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "test".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&work).expect("work completes");
        let complete = plan.milestone_summaries();
        assert!(complete[0].complete);
        assert_eq!(complete[0].completed_work_items, 1);

        let mut store = PlanStore::open_in_memory().expect("store opens");
        store.save(&plan).expect("structured plan saves");
        let restored = store.load(plan.id()).expect("structured plan loads");
        assert_eq!(restored, plan);
        assert!(restored.milestone_summaries()[0].complete);
    }

    #[test]
    fn stale_writer_is_rejected_and_history_is_append_only() {
        let path = std::env::temp_dir().join(format!(
            "mg-plan-conflict-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let plan = Plan::new(id("plan-conflict"), "Conflict plan").expect("plan is valid");
        let mut initial = PlanStore::open(&path).expect("store opens");
        initial.create(&plan).expect("plan creates");
        drop(initial);

        let first = PlanStore::open(&path).expect("first writer opens");
        let second = PlanStore::open(&path).expect("second writer opens");
        let first_loaded = first.load_versioned(plan.id()).expect("first reads");
        let second_loaded = second.load_versioned(plan.id()).expect("second reads");
        assert_eq!(first_loaded.revision, 1);
        assert_eq!(second_loaded.revision, 1);

        let mut first_plan = first_loaded.plan;
        first_plan
            .revise_work_item(&id("missing-work"), "irrelevant")
            .expect_err("missing work is rejected");
        let first_saved = Plan::new(id("plan-conflict"), "First writer").expect("plan is valid");
        let mut first = first;
        let saved = first
            .save_if_revision(&first_saved, first_loaded.revision)
            .expect("first writer saves");
        assert_eq!(saved.revision, 2);

        let second_plan = Plan::new(id("plan-conflict"), "Stale writer").expect("plan is valid");
        let mut second = second;
        assert!(matches!(
            second.save_if_revision(&second_plan, second_loaded.revision),
            Err(StoreError::RevisionConflict {
                id,
                expected: 1,
                actual: 2,
            }) if id == PlanId::new("plan-conflict").expect("plan id is valid")
        ));
        let history = first.history(plan.id()).expect("history reads");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].revision, 1);
        assert_eq!(history[1].revision, 2);
        assert_eq!(
            first.load(plan.id()).expect("current plan loads").title(),
            "First writer"
        );
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn persisted_lifecycle_reopens_downstream_work_after_revision() {
        let mut plan = Plan::new(id("plan-lifecycle"), "Lifecycle plan").expect("plan is valid");
        let prerequisite: WorkItemId = id("work-prerequisite");
        let dependent: WorkItemId = id("work-dependent");
        let prerequisite_criterion: CriterionId = id("criterion-prerequisite");
        let dependent_criterion: CriterionId = id("criterion-dependent");
        plan.add_work_item(prerequisite.clone(), "Establish prerequisite")
            .expect("work is valid");
        plan.add_work_item(dependent.clone(), "Complete dependent")
            .expect("work is valid");
        plan.add_dependency(&dependent, &prerequisite)
            .expect("dependency is valid");
        plan.add_criterion(
            &prerequisite,
            prerequisite_criterion.clone(),
            "Prerequisite is proven",
        )
        .expect("criterion is valid");
        plan.add_criterion(
            &dependent,
            dependent_criterion.clone(),
            "Dependent is proven",
        )
        .expect("criterion is valid");
        let prerequisite_revision = plan
            .work_item(&prerequisite)
            .expect("prerequisite exists")
            .revision;
        let dependent_revision = plan
            .work_item(&dependent)
            .expect("dependent exists")
            .revision;
        plan.record_verification(
            &prerequisite,
            VerificationInput {
                id: id("verification-prerequisite"),
                criterion_id: prerequisite_criterion,
                subject_revision: prerequisite_revision,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "test".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&prerequisite)
            .expect("prerequisite completes");
        plan.record_verification(
            &dependent,
            VerificationInput {
                id: id("verification-dependent"),
                criterion_id: dependent_criterion,
                subject_revision: dependent_revision,
                evidence: vec![evidence()],
                result: VerificationResult::Pass,
                verifier: "test".to_owned(),
            },
        )
        .expect("verification is valid");
        plan.complete(&dependent).expect("dependent completes");

        let path = std::env::temp_dir().join(format!(
            "mg-plan-lifecycle-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let mut store = PlanStore::open(&path).expect("store opens");
        store.save(&plan).expect("completed plan saves");
        drop(store);

        let mut reopened = PlanStore::open(&path).expect("store reopens");
        let mut loaded = reopened.load(plan.id()).expect("completed plan loads");
        assert_eq!(
            loaded
                .work_item(&dependent)
                .expect("dependent exists")
                .status,
            WorkItemStatus::Completed
        );
        loaded
            .revise_work_item(&prerequisite, "Revise prerequisite")
            .expect("revision is valid");
        assert_eq!(
            loaded
                .work_item(&prerequisite)
                .expect("prerequisite exists")
                .status,
            WorkItemStatus::Planned
        );
        assert_eq!(
            loaded
                .work_item(&dependent)
                .expect("dependent exists")
                .status,
            WorkItemStatus::Planned
        );
        reopened.save(&loaded).expect("revised plan saves");
        drop(reopened);

        let final_store = PlanStore::open(&path).expect("store opens after revision");
        let final_plan = final_store.load(plan.id()).expect("revised plan loads");
        assert_eq!(
            final_plan
                .work_item(&dependent)
                .expect("dependent exists")
                .status,
            WorkItemStatus::Planned
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn plan_store_round_trips_complete_aggregate_and_json() {
        let mut plan = Plan::new(id("plan-store"), "Persistent plan").expect("plan is valid");
        let work = id("work-store");
        plan.add_work_item(work, "Persist this work")
            .expect("work is valid");

        let mut store = PlanStore::open_in_memory().expect("store opens");
        store.save(&plan).expect("plan saves");
        let loaded = store.load(plan.id()).expect("plan loads");
        assert_eq!(loaded, plan);

        let document = store.export_json(plan.id()).expect("plan exports");
        let imported_id = store.import_json(&document).expect("plan imports");
        assert_eq!(imported_id, *plan.id());
        assert_eq!(store.load(plan.id()).expect("imported plan loads"), plan);
    }

    #[test]
    fn plan_store_persists_after_reopen() {
        let path = std::env::temp_dir().join(format!(
            "mg-plan-store-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let plan = Plan::new(id("plan-reopen"), "Restart plan").expect("plan is valid");
        let mut first = PlanStore::open(&path).expect("store opens");
        first.save(&plan).expect("plan saves");
        drop(first);
        let second = PlanStore::open(&path).expect("store reopens");
        assert_eq!(second.load(plan.id()).expect("plan loads"), plan);
        drop(second);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn passing_verification_requires_evidence() {
        let mut plan = Plan::new(id("plan-1"), "Evidence test").expect("plan is valid");
        let work: WorkItemId = id("work-1");
        let criterion: CriterionId = id("criterion-1");
        plan.add_work_item(work.clone(), "Work")
            .expect("work is valid");
        plan.add_criterion(&work, criterion.clone(), "The behavior is demonstrated")
            .expect("criterion is valid");
        assert_eq!(
            plan.record_verification(
                &work,
                VerificationInput {
                    id: id("verification-1"),
                    criterion_id: criterion,
                    subject_revision: 2,
                    evidence: Vec::new(),
                    result: VerificationResult::Pass,
                    verifier: "human:jeff".to_owned(),
                },
            ),
            Err(PlanError::PassRequiresEvidence)
        );
    }
}
