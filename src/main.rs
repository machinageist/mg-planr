use std::env;
use std::fs;
use std::process;

use mg_plan::{
    EvidenceId, EvidenceRef, Plan, PlanError, PlanId, PlanStore, VerificationId, VerificationInput,
    VerificationResult, WorkItemId,
};

fn usage() -> &'static str {
    "usage:\n  mg-plan create <db> <plan-id> <title>\n  mg-plan show <db> <plan-id>\n  mg-plan export <db> <plan-id>\n  mg-plan import <db> <json-file>\n  mg-plan add-work <db> <plan-id> <work-id> <title>\n  mg-plan add-dependency <db> <plan-id> <dependent-id> <prerequisite-id>\n  mg-plan add-criterion <db> <plan-id> <work-id> <criterion-id> <statement>\n  mg-plan start|block|unblock <db> <plan-id> <work-id>\n  mg-plan revise <db> <plan-id> <work-id> <title>\n  mg-plan verify <db> <plan-id> <work-id> <verification-id> <criterion-id> <subject-revision> <evidence-id> <producer> <source-record> <evidence-revision> <digest> <pass|fail|inconclusive|waived> <verifier>\n  mg-plan complete <db> <plan-id> <work-id>"
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
    let store = PlanStore::open(db).map_err(|error| error.to_string())?;
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
            let mut store = PlanStore::open(db).map_err(|error| error.to_string())?;
            store.create(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.id());
        }
        "show" | "export" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let id = plan_id(args.next().ok_or_else(|| usage().to_owned())?)?;
            let store = PlanStore::open(db).map_err(|error| error.to_string())?;
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
            let mut store = PlanStore::open(db).map_err(|error| error.to_string())?;
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
