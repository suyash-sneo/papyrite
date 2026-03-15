use engine::Database;
use std::env;

fn main() {

    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        println!("Usage:");
        println!("  put <db> <key> <value>");
        println!("  get <db> <key>");
        println!("  dump <db>");
        return;
    }

    let command = &args[1];
    let db_path = &args[2];

    let db = Database::open(db_path);

    match command.as_str() {
        
        "put" => {
            let key = &args[3];
            let value = &args[4];

            db.put(key, value).unwrap();
        }

        "get" => {
            let key = &args[3];

            match db.get(key).unwrap() {
                Some(v) => println!("{}", v),
                None => println!("Not found"),
            }
        }

        "dump" => {
            db.dump().unwrap();
        }

        _ => {
            println!("unknown command");
        }
    }
}
