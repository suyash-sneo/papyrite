# Papyrite

Papyrite is an embedded, append-only document database implemented in Rust. It stores JSON-like documents in a local database file and exposes both an engine crate and a small CLI.

The current implementation supports:

- Single-file embedded storage
- JSON document creation by required string `_id`
- Read by `_id`
- Delete by `_id`
- Full live-document dump
- Exact-match find on top-level or nested dot paths
- Update with `set` and `unset` operations
- Persistence across reopen
- Unit, integration, and CLI smoke tests

Planned but not yet implemented:

- Multiple collections
- Secondary indexes
- Transactions
- Crash recovery guarantees
- Compaction
- C ABI bindings

## Workspace

This repository is a Rust workspace with two crates:

- `engine`: database storage, record encoding, JSON conversion, query parsing, and document operations
- `cli`: command-line wrapper around the engine JSON API

## Requirements

- Rust toolchain with Cargo

The workspace uses Rust edition `2024`.

## CLI Usage

The CLI accepts a database path followed by one command expression:

```sh
cargo run -p cli -- <db-path> '<command(payload)>'
```

Use quotes around command expressions so your shell does not interpret JSON braces or parentheses.

You can also read the command expression from a file by prefixing the path with `@`:

```sh
cargo run -p cli -- test.db @debug-command.txt
```

For example, `debug-command.txt` can contain:

```text
create({"_id":"u1","name":"Anna"})
```

The file is trimmed before parsing, so trailing newlines are fine. This is useful for IDE debugging because the run/debug configuration can stay fixed while you edit the command file:

```text
run -p cli -- /tmp/papyrite-debug.db @debug-command.txt
```

### Create

Creates a document. The document root must be an object and `_id` must be a string.

```sh
cargo run -p cli -- test.db 'create({"_id":"u1","name":"Anna","profile":{"email":"a@example.com"}})'
```

Output:

```text
ok
```

Creating another live document with the same `_id` fails.

### Get

Reads one live document by `_id`.

```sh
cargo run -p cli -- test.db 'get({"_id":"u1"})'
```

Output is pretty-printed JSON, or `null` if no live document exists:

```json
{
  "_id": "u1",
  "name": "Anna",
  "profile": {
    "email": "a@example.com"
  }
}
```

### Delete

Deletes one live document by `_id`.

```sh
cargo run -p cli -- test.db 'delete({"_id":"u1"})'
```

Output:

```text
true
```

The command prints `false` when the document is missing.

### Update

Updates one live document selected by `_id`.

```sh
cargo run -p cli -- test.db 'update({"filter":{"_id":"u1"},"set":{"name":"Anya","profile.active":true}})'
```

Output:

```text
ok
```

Unset fields with an array of dot paths:

```sh
cargo run -p cli -- test.db 'update({"filter":{"_id":"u1"},"unset":["profile.email"]})'
```

Updating `_id` to a different value or unsetting `_id` is rejected.

### Find

Finds all live documents where a path exactly equals a JSON value.

```sh
cargo run -p cli -- test.db 'find({"path":"profile.active","eq":true})'
```

Output is a pretty-printed JSON array:

```json
[
  {
    "_id": "u1",
    "name": "Anya",
    "profile": {
      "active": true
    }
  }
]
```

Integer and floating-point values are distinct, so `1` does not match `1.0`.

### Dump

Prints all live documents.

```sh
cargo run -p cli -- test.db 'dump()'
```

Output is a pretty-printed JSON array.

## Engine API

Basic usage from Rust:

```rust
use engine::Database;

let db = Database::open("test.db");

db.create_json(r#"{"_id":"u1","name":"Anna"}"#)?;
let doc = db.get_json(r#"{"_id":"u1"}"#)?;
db.update_json(r#"{"filter":{"_id":"u1"},"set":{"name":"Anya"}}"#)?;
let matches = db.find_json(r#"{"path":"name","eq":"Anya"}"#)?;
let deleted = db.delete_json(r#"{"_id":"u1"}"#)?;
```

Lower-level methods are also available for working with `Value` directly:

- `create`
- `get_by_id`
- `delete_by_id`
- `update`
- `find_eq`
- `dump`

## Storage Model

The database file is append-only. Creates and updates append `Put` records, and deletes append `Delete` records. Reads scan records and materialize the latest live state by document `_id`.

## Testing

Run the full suite:

```sh
cargo test
```

Run engine tests only:

```sh
cargo test -p engine
```

Run CLI tests only:

```sh
cargo test -p cli
```

Check formatting:

```sh
cargo fmt --check
```

Current coverage includes:

- Binary value codec round trips and malformed input handling
- Record encoding and decoding
- JSON document create/get/delete/update/find behavior
- Persistence after reopening a database file
- CLI smoke tests for supported commands and invalid command handling
