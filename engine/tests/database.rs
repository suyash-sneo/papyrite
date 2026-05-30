use engine::{Database, DbError, Value};
use tempfile::NamedTempFile;

fn temp_db() -> (NamedTempFile, Database) {
    let file = NamedTempFile::new().unwrap();
    let db = Database::open(file.path());
    (file, db)
}

fn json_eq(actual: &str, expected: &str) {
    let actual: serde_json::Value = serde_json::from_str(actual).unwrap();
    let expected: serde_json::Value = serde_json::from_str(expected).unwrap();
    assert_eq!(actual, expected);
}

fn create_user(db: &Database) {
    db.create_json(
        r#"{"_id":"u1","name":"Anna","profile":{"email":"a@example.com"},"obsoleteField":true}"#,
    )
    .unwrap();
}

#[test]
fn create_then_get_by_id() {
    let (_file, db) = temp_db();
    create_user(&db);

    let doc = db.get_by_id("u1").unwrap().unwrap();
    assert_eq!(doc.id_str(), Some("u1"));
    assert_eq!(
        doc.get_path("profile.email"),
        Some(&Value::String("a@example.com".to_string()))
    );
}

#[test]
fn create_duplicate_id_fails() {
    let (_file, db) = temp_db();
    create_user(&db);

    let err = db
        .create_json(r#"{"_id":"u1","name":"Other"}"#)
        .unwrap_err();
    assert!(matches!(err, DbError::DuplicateId(id) if id == "u1"));
}

#[test]
fn delete_existing_returns_true() {
    let (_file, db) = temp_db();
    create_user(&db);

    assert!(db.delete_by_id("u1").unwrap());
}

#[test]
fn delete_missing_returns_false() {
    let (_file, db) = temp_db();
    assert!(!db.delete_by_id("missing").unwrap());
}

#[test]
fn get_after_delete_returns_none() {
    let (_file, db) = temp_db();
    create_user(&db);
    db.delete_by_id("u1").unwrap();

    assert_eq!(db.get_by_id("u1").unwrap(), None);
}

#[test]
fn reopen_and_get_returns_persisted_document() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_path_buf();

    Database::open(&path)
        .create_json(r#"{"_id":"u1","name":"Anna"}"#)
        .unwrap();

    let reopened = Database::open(&path);
    let doc = reopened.get_by_id("u1").unwrap().unwrap();
    assert_eq!(
        doc.get_path("name"),
        Some(&Value::String("Anna".to_string()))
    );
}

#[test]
fn dump_returns_all_live_docs_only() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u2","name":"B"}"#).unwrap();
    db.create_json(r#"{"_id":"u1","name":"A"}"#).unwrap();
    db.delete_by_id("u2").unwrap();

    let docs = db.dump().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id_str(), Some("u1"));
}

#[test]
fn update_replaces_document_logically() {
    let (_file, db) = temp_db();
    create_user(&db);

    db.update_json(r#"{"filter":{"_id":"u1"},"set":{"name":"Anya"}}"#)
        .unwrap();

    let doc = db.get_by_id("u1").unwrap().unwrap();
    assert_eq!(
        doc.get_path("name"),
        Some(&Value::String("Anya".to_string()))
    );
}

#[test]
fn update_with_nested_set_path_works() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","profile":{}}"#).unwrap();

    db.update_json(
        r#"{"filter":{"_id":"u1"},"set":{"profile.email":"new@example.com","profile.flags.active":true}}"#,
    )
    .unwrap();

    let doc = db.get_by_id("u1").unwrap().unwrap();
    assert_eq!(
        doc.get_path("profile.email"),
        Some(&Value::String("new@example.com".to_string()))
    );
    assert_eq!(
        doc.get_path("profile.flags.active"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn update_with_unset_path_works() {
    let (_file, db) = temp_db();
    create_user(&db);

    db.update_json(r#"{"filter":{"_id":"u1"},"unset":["obsoleteField","profile.missing"]}"#)
        .unwrap();

    let doc = db.get_by_id("u1").unwrap().unwrap();
    assert_eq!(doc.get_path("obsoleteField"), None);
}

#[test]
fn update_missing_document_fails() {
    let (_file, db) = temp_db();
    let err = db
        .update_json(r#"{"filter":{"_id":"u1"},"set":{"name":"Anya"}}"#)
        .unwrap_err();
    assert!(matches!(err, DbError::NotFound(id) if id == "u1"));
}

#[test]
fn find_exact_on_top_level_field_works() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","name":"Anna"}"#).unwrap();
    db.create_json(r#"{"_id":"u2","name":"Ben"}"#).unwrap();

    let docs = db.find_json(r#"{"path":"name","eq":"Anna"}"#).unwrap();
    json_eq(&docs, r#"[{"_id":"u1","name":"Anna"}]"#);
}

#[test]
fn find_exact_on_nested_field_works() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","profile":{"email":"a@example.com"}}"#)
        .unwrap();
    db.create_json(r#"{"_id":"u2","profile":{"email":"b@example.com"}}"#)
        .unwrap();

    let docs = db
        .find_json(r#"{"path":"profile.email","eq":"a@example.com"}"#)
        .unwrap();
    json_eq(
        &docs,
        r#"[{"_id":"u1","profile":{"email":"a@example.com"}}]"#,
    );
}

#[test]
fn find_returns_empty_array_when_no_matches() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","name":"Anna"}"#).unwrap();

    let docs = db.find_json(r#"{"path":"name","eq":"Nope"}"#).unwrap();
    json_eq(&docs, "[]");
}

#[test]
fn create_with_non_object_root_fails() {
    let (_file, db) = temp_db();
    let err = db.create_json(r#"["not","an","object"]"#).unwrap_err();
    assert!(matches!(err, DbError::InvalidRootDocument));
}

#[test]
fn create_without_id_fails() {
    let (_file, db) = temp_db();
    let err = db.create_json(r#"{"name":"Anna"}"#).unwrap_err();
    assert!(matches!(err, DbError::MissingId));
}

#[test]
fn create_with_non_string_id_fails() {
    let (_file, db) = temp_db();
    let err = db.create_json(r#"{"_id":1,"name":"Anna"}"#).unwrap_err();
    assert!(matches!(err, DbError::InvalidData(_)));
}

#[test]
fn update_attempting_to_remove_id_fails() {
    let (_file, db) = temp_db();
    create_user(&db);

    let err = db
        .update_json(r#"{"filter":{"_id":"u1"},"unset":["_id"]}"#)
        .unwrap_err();
    assert!(matches!(err, DbError::InvalidData(_)));
}

#[test]
fn update_attempting_to_change_id_fails() {
    let (_file, db) = temp_db();
    create_user(&db);

    let err = db
        .update_json(r#"{"filter":{"_id":"u1"},"set":{"_id":"u2"}}"#)
        .unwrap_err();
    assert!(matches!(err, DbError::InvalidData(_)));
}

#[test]
fn json_api_get_delete_and_dump_work() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","name":"Anna"}"#).unwrap();

    let doc = db.get_json(r#"{"_id":"u1"}"#).unwrap().unwrap();
    json_eq(&doc, r#"{"_id":"u1","name":"Anna"}"#);

    let dump = db.dump_json().unwrap();
    json_eq(&dump, r#"[{"_id":"u1","name":"Anna"}]"#);

    assert!(db.delete_json(r#"{"_id":"u1"}"#).unwrap());
}

#[test]
fn find_int_and_float_do_not_match() {
    let (_file, db) = temp_db();
    db.create_json(r#"{"_id":"u1","value":1}"#).unwrap();
    db.create_json(r#"{"_id":"u2","value":1.0}"#).unwrap();

    let docs = db.find_eq("value", &Value::Int(1)).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id_str(), Some("u1"));
}

#[test]
fn helper_accepts_path_creation() {
    let mut value = Value::from_json_str(r#"{"_id":"u1"}"#).unwrap();
    engine::set_path(
        &mut value,
        "profile.email",
        Value::String("a@example.com".to_string()),
    )
    .unwrap();
    assert_eq!(
        value.get_path("profile.email"),
        Some(&Value::String("a@example.com".to_string()))
    );
}
