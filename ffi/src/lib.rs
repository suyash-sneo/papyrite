use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{ptr, slice, str};

use engine::Database;

pub const PAPYRITE_STATUS_OK: i32 = 0;
pub const PAPYRITE_STATUS_INVALID_ARGUMENT: i32 = 1;
pub const PAPYRITE_STATUS_ENGINE_ERROR: i32 = 2;
pub const PAPYRITE_STATUS_PANIC: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PapyriteBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

impl PapyriteBuffer {
    fn empty() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct PapyriteResult {
    pub code: i32,
    pub data: PapyriteBuffer,
    pub error: PapyriteBuffer,
    pub bool_value: u8,
}

impl PapyriteResult {
    fn ok() -> Self {
        Self {
            code: PAPYRITE_STATUS_OK,
            data: PapyriteBuffer::empty(),
            error: PapyriteBuffer::empty(),
            bool_value: 0,
        }
    }

    fn error(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            data: PapyriteBuffer::empty(),
            error: buffer_from_bytes(message.into().into_bytes()),
            bool_value: 0,
        }
    }
}

enum FfiError {
    InvalidArgument(String),
    Engine(String),
}

type FfiResult<T> = std::result::Result<T, FfiError>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_create_json(
    path_ptr: *const u8,
    path_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let json = unsafe { read_str(json_ptr, json_len, "json")? };
        Database::open(path)
            .create_json(json)
            .map_err(|err| FfiError::Engine(err.to_string()))?;
        Ok(PapyriteResult::ok())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_get_json(
    path_ptr: *const u8,
    path_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let json = unsafe { read_str(json_ptr, json_len, "json")? };
        let doc = Database::open(path)
            .get_json(json)
            .map_err(|err| FfiError::Engine(err.to_string()))?;

        let mut result = PapyriteResult::ok();
        if let Some(doc) = doc {
            result.bool_value = 1;
            result.data = buffer_from_bytes(doc.into_bytes());
        }
        Ok(result)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_delete_json(
    path_ptr: *const u8,
    path_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let json = unsafe { read_str(json_ptr, json_len, "json")? };
        let deleted = Database::open(path)
            .delete_json(json)
            .map_err(|err| FfiError::Engine(err.to_string()))?;

        let mut result = PapyriteResult::ok();
        result.bool_value = u8::from(deleted);
        Ok(result)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_update_json(
    path_ptr: *const u8,
    path_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let json = unsafe { read_str(json_ptr, json_len, "json")? };
        Database::open(path)
            .update_json(json)
            .map_err(|err| FfiError::Engine(err.to_string()))?;
        Ok(PapyriteResult::ok())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_find_json(
    path_ptr: *const u8,
    path_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let json = unsafe { read_str(json_ptr, json_len, "json")? };
        let docs = Database::open(path)
            .find_json(json)
            .map_err(|err| FfiError::Engine(err.to_string()))?;

        let mut result = PapyriteResult::ok();
        result.data = buffer_from_bytes(docs.into_bytes());
        Ok(result)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_dump_json(
    path_ptr: *const u8,
    path_len: usize,
    out: *mut PapyriteResult,
) -> i32 {
    run_ffi(out, || {
        let path = unsafe { read_str(path_ptr, path_len, "path")? };
        let docs = Database::open(path)
            .dump_json()
            .map_err(|err| FfiError::Engine(err.to_string()))?;

        let mut result = PapyriteResult::ok();
        result.data = buffer_from_bytes(docs.into_bytes());
        Ok(result)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_buffer_free(ptr: *mut u8, len: usize) {
    unsafe { free_buffer(PapyriteBuffer { ptr, len }) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn papyrite_result_free(result: *mut PapyriteResult) {
    if result.is_null() {
        return;
    }

    let result = unsafe { &mut *result };
    unsafe {
        free_buffer(result.data);
        free_buffer(result.error);
    }

    result.code = PAPYRITE_STATUS_OK;
    result.data = PapyriteBuffer::empty();
    result.error = PapyriteBuffer::empty();
    result.bool_value = 0;
}

fn run_ffi<F>(out: *mut PapyriteResult, f: F) -> i32
where
    F: FnOnce() -> FfiResult<PapyriteResult>,
{
    if out.is_null() {
        return PAPYRITE_STATUS_INVALID_ARGUMENT;
    }

    let result = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(result)) => result,
        Ok(Err(FfiError::InvalidArgument(message))) => {
            PapyriteResult::error(PAPYRITE_STATUS_INVALID_ARGUMENT, message)
        }
        Ok(Err(FfiError::Engine(message))) => {
            PapyriteResult::error(PAPYRITE_STATUS_ENGINE_ERROR, message)
        }
        Err(_) => PapyriteResult::error(
            PAPYRITE_STATUS_PANIC,
            "panic caught inside Papyrite FFI boundary",
        ),
    };

    let code = result.code;
    unsafe { ptr::write(out, result) };
    code
}

unsafe fn read_str<'a>(ptr: *const u8, len: usize, name: &str) -> FfiResult<&'a str> {
    if ptr.is_null() {
        if len == 0 {
            return Ok("");
        }
        return Err(FfiError::InvalidArgument(format!(
            "{name} pointer is null but length is {len}"
        )));
    }

    let bytes = unsafe { slice::from_raw_parts(ptr, len) };
    str::from_utf8(bytes)
        .map_err(|err| FfiError::InvalidArgument(format!("{name} is not valid UTF-8: {err}")))
}

fn buffer_from_bytes(bytes: Vec<u8>) -> PapyriteBuffer {
    if bytes.is_empty() {
        return PapyriteBuffer::empty();
    }

    let len = bytes.len();
    let ptr = Box::into_raw(bytes.into_boxed_slice()) as *mut u8;
    PapyriteBuffer { ptr, len }
}

unsafe fn free_buffer(buffer: PapyriteBuffer) {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return;
    }

    let slice = ptr::slice_from_raw_parts_mut(buffer.ptr, buffer.len);
    unsafe {
        drop(Box::from_raw(slice));
    }
}

#[cfg(test)]
mod tests {
    use std::mem::MaybeUninit;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn create_get_delete_round_trip_through_abi() {
        let db = NamedTempFile::new().unwrap();

        let create = call_json(
            papyrite_create_json,
            db.path().to_str().unwrap(),
            r#"{"_id":"u1","name":"Anna"}"#,
        );
        assert_eq!(create.code, PAPYRITE_STATUS_OK);

        let get = call_json(
            papyrite_get_json,
            db.path().to_str().unwrap(),
            r#"{"_id":"u1"}"#,
        );
        assert_eq!(get.code, PAPYRITE_STATUS_OK);
        assert!(get.bool_value);
        assert_eq!(get.data["name"], "Anna");

        let delete = call_json(
            papyrite_delete_json,
            db.path().to_str().unwrap(),
            r#"{"_id":"u1"}"#,
        );
        assert_eq!(delete.code, PAPYRITE_STATUS_OK);
        assert!(delete.bool_value);

        let missing = call_json(
            papyrite_get_json,
            db.path().to_str().unwrap(),
            r#"{"_id":"u1"}"#,
        );
        assert_eq!(missing.code, PAPYRITE_STATUS_OK);
        assert!(!missing.bool_value);
        assert!(missing.data.is_null());
    }

    #[test]
    fn update_find_and_dump_through_abi() {
        let db = NamedTempFile::new().unwrap();
        let path = db.path().to_str().unwrap();

        assert_eq!(
            call_json(
                papyrite_create_json,
                path,
                r#"{"_id":"u1","name":"Anna","profile":{}}"#
            )
            .code,
            PAPYRITE_STATUS_OK
        );
        assert_eq!(
            call_json(
                papyrite_update_json,
                path,
                r#"{"filter":{"_id":"u1"},"set":{"profile.active":true}}"#
            )
            .code,
            PAPYRITE_STATUS_OK
        );

        let found = call_json(
            papyrite_find_json,
            path,
            r#"{"path":"profile.active","eq":true}"#,
        );
        assert_eq!(found.code, PAPYRITE_STATUS_OK);
        assert_eq!(found.data.as_array().unwrap().len(), 1);

        let dumped = call_dump(path);
        assert_eq!(dumped.code, PAPYRITE_STATUS_OK);
        assert_eq!(dumped.data.as_array().unwrap().len(), 1);
    }

    #[test]
    fn invalid_arguments_return_error_instead_of_panicking() {
        let mut out = MaybeUninit::<PapyriteResult>::uninit();
        let code = unsafe {
            papyrite_create_json(std::ptr::null(), 1, std::ptr::null(), 0, out.as_mut_ptr())
        };

        assert_eq!(code, PAPYRITE_STATUS_INVALID_ARGUMENT);
        let mut result = unsafe { out.assume_init() };
        assert_eq!(result.code, PAPYRITE_STATUS_INVALID_ARGUMENT);
        assert!(!read_error(&result).is_empty());
        unsafe { papyrite_result_free(&mut result) };
    }

    #[test]
    fn null_result_pointer_returns_invalid_argument_code() {
        let path = b"/tmp/papyrite.db";
        let json = br#"{"_id":"u1"}"#;

        let code = unsafe {
            papyrite_create_json(
                path.as_ptr(),
                path.len(),
                json.as_ptr(),
                json.len(),
                std::ptr::null_mut(),
            )
        };

        assert_eq!(code, PAPYRITE_STATUS_INVALID_ARGUMENT);
    }

    struct OwnedResult {
        code: i32,
        bool_value: bool,
        data: serde_json::Value,
    }

    fn call_json(
        f: unsafe extern "C" fn(*const u8, usize, *const u8, usize, *mut PapyriteResult) -> i32,
        path: &str,
        json: &str,
    ) -> OwnedResult {
        let mut out = MaybeUninit::<PapyriteResult>::uninit();
        let code = unsafe {
            f(
                path.as_ptr(),
                path.len(),
                json.as_ptr(),
                json.len(),
                out.as_mut_ptr(),
            )
        };
        let mut result = unsafe { out.assume_init() };
        assert_eq!(code, result.code);
        let data = read_json_data(&result);
        let bool_value = result.bool_value != 0;
        unsafe { papyrite_result_free(&mut result) };
        OwnedResult {
            code,
            bool_value,
            data,
        }
    }

    fn call_dump(path: &str) -> OwnedResult {
        let mut out = MaybeUninit::<PapyriteResult>::uninit();
        let code = unsafe { papyrite_dump_json(path.as_ptr(), path.len(), out.as_mut_ptr()) };
        let mut result = unsafe { out.assume_init() };
        assert_eq!(code, result.code);
        let data = read_json_data(&result);
        let bool_value = result.bool_value != 0;
        unsafe { papyrite_result_free(&mut result) };
        OwnedResult {
            code,
            bool_value,
            data,
        }
    }

    fn read_json_data(result: &PapyriteResult) -> serde_json::Value {
        if result.data.ptr.is_null() || result.data.len == 0 {
            return serde_json::Value::Null;
        }

        let bytes = unsafe { slice::from_raw_parts(result.data.ptr, result.data.len) };
        serde_json::from_slice(bytes).unwrap()
    }

    fn read_error(result: &PapyriteResult) -> String {
        if result.error.ptr.is_null() || result.error.len == 0 {
            return String::new();
        }

        let bytes = unsafe { slice::from_raw_parts(result.error.ptr, result.error.len) };
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
