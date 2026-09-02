use std::env;
use std::fs;
use std::process;

use mg_plan::{Plan, PlanId, PlanStore};

fn usage() -> &'static str {
    "usage:\n  mg-plan create <db> <plan-id> <title>\n  mg-plan show <db> <plan-id>\n  mg-plan export <db> <plan-id>\n  mg-plan import <db> <json-file>"
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut args = args;
    let command = args.next().ok_or_else(|| usage().to_owned())?;
    match command.as_str() {
        "create" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let id = PlanId::new(args.next().ok_or_else(|| usage().to_owned())?)
                .map_err(|error| error.to_string())?;
            let title = args.collect::<Vec<_>>().join(" ");
            if title.is_empty() {
                return Err(usage().to_owned());
            }
            let plan = Plan::new(id, title).map_err(|error| error.to_string())?;
            let mut store = PlanStore::open(db).map_err(|error| error.to_string())?;
            store.save(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.id());
        }
        "show" | "export" => {
            let db = args.next().ok_or_else(|| usage().to_owned())?;
            let id = PlanId::new(args.next().ok_or_else(|| usage().to_owned())?)
                .map_err(|error| error.to_string())?;
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
            let id = store
                .import_json(&document)
                .map_err(|error| error.to_string())?;
            println!("{}", id);
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
