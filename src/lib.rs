use libc::{c_int, EACCES, EBUSY, EXIT_SUCCESS};
use snafu::{ResultExt, Snafu};
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::ptr::null_mut;

use rusqlite::ffi::sqlite3;
use rusqlite::{params, Connection, Error};

#[derive(Debug)]
struct KeyMode {
    key: String,
    mode: i32,
}

#[repr(i32)]
#[derive(Snafu, Debug)]
pub enum ReadDirError {
    EAcess { source: Error },
    EBusy,
}
impl From<ReadDirError> for c_int {
    fn from(err: ReadDirError) -> Self {
        match err {
            ReadDirError::EBusy { .. } => EBUSY,
            ReadDirError::EAcess { .. } => EACCES,
        }
    }
}

#[no_mangle]
pub extern "C" fn readdir(
    handle: *mut sqlite3,
    path_ptr: *const c_char,
    filler_buff: *mut c_void,
    filler: unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_void, i32) -> i32,
) -> c_int {
    // Convert the sqlite3 pointer to a Connection.
    let connection = unsafe { Connection::from_handle(handle) }
        .expect("todo: couldn't open Connection from handle");

    // Convert the path_ptr to a rust &str
    let path = unsafe { CStr::from_ptr(path_ptr) };
    let path = path
        .to_str()
        .expect("todo: couldn't convert path to valid utf8 rust str");

    // Run our query
    let result = readdir_(connection, path, |st| {
        let s = CString::new(st).unwrap();

        unsafe { filler(filler_buff, s.as_ptr(), null_mut(), 0) };
    });

    // Map the result into libc codes
    match result {
        Err(e) => e.into(),
        Ok(_) => EXIT_SUCCESS,
    }
}

/// Internal method with no unsafe code.
///
/// Never panics
fn readdir_<F: FnMut(&str)>(
    connection: Connection,
    path: &str,
    mut cb: F,
) -> Result<(), ReadDirError> {
    // remove any leading slashes
    let path = path.trim_start_matches("/");

    // format the glob pattern
    let glob = format!("{}/*", path);

    // Prepare the query
    let mut stmt = connection
        .prepare("select key, mode from meta_data where key glob ?1;")
        .context(EAcess)?;

    // Actually do the query
    let key_mode_iter = stmt
        .query_map(params![glob], |row| {
            Ok(KeyMode {
                key: row.get(0)?,
                mode: row.get(1)?,
            })
        })
        .context(EAcess)?;

    // Some results need to be filtered out
    let mut filtered_iter = key_mode_iter.filter(|key_mode| {
        match key_mode {
            // Skip if grandchild etc
            Ok(key_mode) if key_mode.key == "/" => false,
            // Skip if result is path
            Ok(key_mode) if key_mode.key == path => false,
            // Special case, skip when dir the root dir
            Ok(key_mode) if key_mode.mode == 0 => false,
            _ => true,
        }
    });

    // Part of the contract is that we always return these dirs
    cb(".");
    cb("..");

    // If any loop returns an Err then we return that error immediately using `try_for_each`.
    filtered_iter.try_for_each(|key_mode| {
        match key_mode {
            Ok(key_mode) => Ok(cb(&key_mode.key)),
            Err(_) => Err(ReadDirError::EBusy), //todo
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
