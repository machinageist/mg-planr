use std::env;
use std::fs;
use std::process;

use mg_plan::{
    EvidenceId, EvidenceRef, Plan, PlanError, PlanId, PlanStore, VerificationId, VerificationInput,
    VerificationResult, WorkItemId,
};

fn usage() -> &'static str {
    "usage:\n  mg-plan create <database-url> <plan-id> <title>\n  mg-plan show <database-url> <plan-id>\n  mg-plan export <database-url> <plan-id>\n  mg-plan import <database-url> <json-file>\n  mg-plan add-work <database-url> <plan-id> <work-id> <title>\n  mg-plan add-dependency <database-url> <plan-id> <dependent-id> <prerequisite-id>\n  mg-plan add-criterion <database-url> <plan-id> <work-id> <criterion-id> <statement>\n  mg-plan start|block|unblock <database-url> <plan-id> <work-id>\n  mg-plan revise <database-url> <plan-id> <work-id> <title>\n  mg-plan verify <database-url> <plan-id> <work-id> <verification-id> <criterion-id> <subject-revision> <evidence-id> <producer> <source-record> <evidence-revision> <digest> <pass|fail|inconclusive|waived> <verifier>\n  mg-plan complete <database-url> <plan-id> <work-id>\n  mg-plan list-work <database-url> <plan-id>\n  mg-plan blocked <database-url> <plan-id>\n  mg-plan verification-gaps <database-url> <plan-id>\n  mg-plan add-project <database-url> <plan-id> <project-id> <title>\n  mg-plan add-milestone <database-url> <plan-id> <project-id> <milestone-id> <title>\n  mg-plan link-work <database-url> <plan-id> <milestone-id> <work-id>\n  mg-plan decide <database-url> <plan-id> <decision-id> <question> <decision> <rationale>\n  mg-plan milestones <database-url> <plan-id>\n  mg-plan schedule-request <database-url> <plan-id> <request-id> <work-id> <calendar> <requested-start> <duration-minutes>\n  mg-plan schedule-receipt <database-url> <plan-id> <request-id> <event-id> <calendar> <event-revision>\n  mg-plan schedules <database-url> <plan-id>"
}

fn plan_id(value: String) -> Result<PlanId, String> {
    PlanId::new(value).map_err(|error| error.to_string())
}

fn work_id(value: String) -> Result<WorkItemId, String> {
    WorkItemId::new(value).map_err(|error| error.to_string())
}

fn verification_id(value: String) -> Result<VerificationId, String> {
    VerificationId::new(value).map_err(|error| error.to_string())
}

fn evidence_id(value: String) -> Result<EvidenceId, String> {
    EvidenceId::new(value).map_err(|error| error.to_string())
}

fn number(value: String, field: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("{field} must be an unsigned integer"))
}

fn result(value: String) -> Result<VerificationResult, String> {
    match value.as_str() {
        "pass" => Ok(VerificationResult::Pass),
        "fail" => Ok(VerificationResult::Fail),
        "inconclusive" => Ok(VerificationResult::Inconclusive),
        "waived" => Ok(VerificationResult::Waived),
        _ => Err("verification result must be pass, fail, inconclusive, or waived".to_owned()),
    }
}

fn load_plan(db: String, id: String) -> Result<(PlanStore, Plan, u64), String> {
    let id = plan_id(id)?;
    let mut store = PlanStore::open(&db).map_err(|error| error.to_string())?;
    let stored = store
        .load_versioned(&id)
        .map_err(|error| error.to_string())?;
    Ok((store, stored.plan, stored.revision))
}

fn domain(error: PlanError) -> String {
    error.to_string()
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args;
    let command = args.next().ok_or_else(|| usage().to_owned())?;
    match command.as_str() {
        "create" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let id = plan_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let title = args.collect::<Vec<_>>().join(" ");
            let plan = Plan::new(id, title).map_err(domain)?;
            let mut store = PlanStore::open(&db).map_err(|error| error.to_string())?;
            store.create(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.id());
        }
        "show" | "export" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let id = plan_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let mut store = PlanStore::open(&db).map_err(|error| error.to_string())?;
            println!(
                "{}",
                store.export_json(&id).map_err(|error| error.to_string())?
            );
        }
        "import" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let json_file = args.next().ok_or_else(|| usage().to_owned())?;
            let document =
                fs::read_to_string(json_file).map_err(|_| "could not read JSON file".to_owned())?;
            let mut store = PlanStore::open(&db).map_err(|error| error.to_string())?;
            println!(
                "{}",
                store
                    .import_json(&document)
                    .map_err(|error| error.to_string())?
            );
        }
        "add-work" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let id = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let title = args.collect::<Vec<_>>().join(" ");
            plan.add_work_item(id.clone(), title).map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
            println!("{}", id);
        }
        "add-dependency" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let dependent = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let prerequisite = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            plan.add_dependency(&dependent, &prerequisite)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "add-criterion" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let criterion =
                mg_plan::CriterionId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let statement = args.collect::<Vec<_>>().join(" ");
            plan.add_criterion(&work, criterion, statement)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "start" | "block" | "unblock" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            match command.as_str() {
                "start" => plan.start_work(&work),
                "block" => plan.block_work(&work),
                _ => plan.unblock_work(&work),
            }
            .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "revise" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let title = args.collect::<Vec<_>>().join(" ");
            plan.revise_work_item(&work, title).map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "verify" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let verification = verification_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let criterion =
                mg_plan::CriterionId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let subject_revision = number(
                args.next().ok_or_else(|| usage().to_owned())?,
                "subject revision",
            )?;
            let evidence = EvidenceRef::new(
                evidence_id(args.next().ok_or_else(|| usage().to_owned())?)?,
                args.next().ok_or_else(|| usage().to_owned())?,
                args.next().ok_or_else(|| usage().to_owned())?,
                args.next().ok_or_else(|| usage().to_owned())?,
                args.next().ok_or_else(|| usage().to_owned())?,
            )
            .map_err(domain)?;
            let verification_result = result(args.next().ok_or_else(|| usage().to_owned())?)?;
            let verifier = args.collect::<Vec<_>>().join(" ");
            plan.record_verification(
                &work,
                VerificationInput {
                    id: verification,
                    criterion_id: criterion,
                    subject_revision,
                    evidence: vec![evidence],
                    result: verification_result,
                    verifier,
                },
            )
            .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "complete" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            plan.complete(&work).map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "schedule-request" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let request =
                mg_plan::ScheduleRequestId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let calendar = args.next().ok_or_else(|| usage().to_owned())?;
            let start = args.next().ok_or_else(|| usage().to_owned())?;
            let duration = number(args.next().ok_or_else(|| usage().to_owned())?, "duration")?;
            plan.request_schedule(request.clone(), &work, calendar, start, duration)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
            println!("{request}");
        }
        "schedule-receipt" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let request =
                mg_plan::ScheduleRequestId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let event =
                mg_plan::CalendarEventId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let calendar = args.next().ok_or_else(|| usage().to_owned())?;
            let event_revision = args.next().ok_or_else(|| usage().to_owned())?;
            plan.record_schedule_receipt(&request, event, calendar, event_revision)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "schedules" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (_, plan, _) = load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan.schedule_summaries())
                    .map_err(|_| "could not serialize schedule result".to_owned())?
            );
        }
        "add-project" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let project = mg_plan::ProjectId::new(args.next().ok_or_else(|| usage().to_owned())?)
                .map_err(|error| error.to_string())?;
            let title = args.collect::<Vec<_>>().join(" ");
            plan.add_project(project.clone(), title).map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
            println!("{project}");
        }
        "add-milestone" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let project = mg_plan::ProjectId::new(args.next().ok_or_else(|| usage().to_owned())?)
                .map_err(|error| error.to_string())?;
            let milestone =
                mg_plan::MilestoneId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let title = args.collect::<Vec<_>>().join(" ");
            plan.add_milestone(&project, milestone.clone(), title)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
            println!("{milestone}");
        }
        "link-work" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let milestone =
                mg_plan::MilestoneId::new(args.next().ok_or_else(|| usage().to_owned())?)
                    .map_err(|error| error.to_string())?;
            let work = work_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            plan.link_work_item_to_milestone(&milestone, &work)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "decide" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (mut store, mut plan, revision) =
                load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let decision = mg_plan::DecisionId::new(args.next().ok_or_else(|| usage().to_owned())?)
                .map_err(|error| error.to_string())?;
            let question = args.next().ok_or_else(|| usage().to_owned())?;
            let decision_text = args.next().ok_or_else(|| usage().to_owned())?;
            let rationale = args.collect::<Vec<_>>().join(" ");
            plan.record_decision(decision, question, decision_text, rationale)
                .map_err(domain)?;
            store
                .save_if_revision(&plan, revision)
                .map_err(|error| error.to_string())?;
        }
        "milestones" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (_, plan, _) = load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan.milestone_summaries())
                    .map_err(|_| "could not serialize milestone result".to_owned())?
            );
        }
        "list-work" | "blocked" | "verification-gaps" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let (_, plan, _) = load_plan(db, args.next().ok_or_else(|| usage().to_owned())?)?;
            let document = match command.as_str() {
                "list-work" => serde_json::to_string_pretty(&plan.work_item_summaries()),
                "blocked" => serde_json::to_string_pretty(&plan.blocked_work_item_summaries()),
                _ => serde_json::to_string_pretty(&plan.verification_gaps()),
            }
            .map_err(|_| "could not serialize query result".to_owned())?;
            println!("{document}");
        }
        "help" | "--help" | "-h" => println!("{}", usage()),
        _ => return Err(usage().to_owned()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("error: {error}");
        process::exit(2);
    }
}
