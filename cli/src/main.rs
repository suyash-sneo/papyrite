use std::env;
use std::process;

use engine::Database;

enum Command<'a> {
    Create(&'a str),
    Get(&'a str),
    Delete(&'a str),
    Update(&'a str),
    Find(&'a str),
    Dump,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();
    let _program = args.next();
    let db_path = args
        .next()
        .ok_or_else(|| "usage: cli <db-path> '<command(...)>'".to_string())?;
    let command_str = args
        .next()
        .ok_or_else(|| "usage: cli <db-path> '<command(...)>'".to_string())?;

    if args.next().is_some() {
        return Err("usage: cli <db-path> '<command(...)>'".to_string());
    }

    let db = Database::open(db_path);
    match parse_command(&command_str)? {
        Command::Create(payload) => {
            db.create_json(payload).map_err(|err| err.to_string())?;
            println!("ok");
        }
        Command::Get(payload) => match db.get_json(payload).map_err(|err| err.to_string())? {
            Some(doc) => println!("{doc}"),
            None => println!("null"),
        },
        Command::Delete(payload) => {
            let deleted = db.delete_json(payload).map_err(|err| err.to_string())?;
            println!("{deleted}");
        }
        Command::Update(payload) => {
            db.update_json(payload).map_err(|err| err.to_string())?;
            println!("ok");
        }
        Command::Find(payload) => {
            let docs = db.find_json(payload).map_err(|err| err.to_string())?;
            println!("{docs}");
        }
        Command::Dump => {
            let docs = db.dump_json().map_err(|err| err.to_string())?;
            println!("{docs}");
        }
    }

    Ok(())
}

fn parse_command(input: &str) -> Result<Command<'_>, String> {
    let input = input.trim();

    if input == "dump()" {
        return Ok(Command::Dump);
    }

    if let Some(payload) = parse_payload(input, "create(")? {
        return Ok(Command::Create(payload));
    }
    if let Some(payload) = parse_payload(input, "get(")? {
        return Ok(Command::Get(payload));
    }
    if let Some(payload) = parse_payload(input, "delete(")? {
        return Ok(Command::Delete(payload));
    }
    if let Some(payload) = parse_payload(input, "update(")? {
        return Ok(Command::Update(payload));
    }
    if let Some(payload) = parse_payload(input, "find(")? {
        return Ok(Command::Find(payload));
    }

    Err("unknown command".to_string())
}

fn parse_payload<'a>(input: &'a str, prefix: &str) -> Result<Option<&'a str>, String> {
    let Some(rest) = input.strip_prefix(prefix) else {
        return Ok(None);
    };

    let payload = rest
        .strip_suffix(')')
        .ok_or_else(|| "command must end with ')'".to_string())?
        .trim();
    if payload.is_empty() {
        return Err("command payload must not be empty".to_string());
    }

    Ok(Some(payload))
}
