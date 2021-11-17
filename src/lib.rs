use libc::{c_int, EACCES, EBUSY, EXIT_SUCCESS};
pub use libfuse_sys::fuse::fuse_fill_dir_t;
use snafu::{ResultExt, Snafu};
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;
use std::ptr::null_mut;

use rusqlite::ffi::sqlite3;
use rusqlite::{params, Connection, Error};

#[repr(i32)]
#[derive(Snafu, Debug)]
pub enum ReadDirError {
    EAcess { source: Error },
    EBusy { source: Error },
}

// Implement a conversion from our error type to libc error codes.
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
    filler: fuse_fill_dir_t,
) -> c_int {
    // Convert the sqlite3 pointer to a Connection.
    let connection =
        unsafe { Connection::from_handle(handle) }.expect("couldn't open Connection from handle");

    // Convert the path_ptr to a rust &str
    let path = unsafe { CStr::from_ptr(path_ptr) }
        .to_str()
        .expect("couldn't convert path to valid utf8 rust str");

    // Run our query
    let result = readdir_(connection, path, |st| {
        // We need to allocate a new CString here so that it's null terminated. (Rust strings are
        // not null terminated)
        // That's one more allocation per row than C has to do unfortunately.
        let s = CString::new(st).unwrap();

        // Avoid calling a null function pointer
        if let Some(filler) = filler {
            // Call our callback
            unsafe { filler(filler_buff, s.as_ptr(), null_mut(), 0) };
        }
    });

    // Map the result into libc codes
    match result {
        // into uses our conversion above
        Err(e) => e.into(),
        Ok(_) => EXIT_SUCCESS,
    }
}

/// Internal method with no unsafe code that never panics.
///
/// Ok, it could panic if what's in the key column is not text but some other sql type.
fn readdir_<F: FnMut(&str)>(
    connection: Connection,
    path: &str,
    mut cb: F,
) -> Result<(), ReadDirError> {
    // remove any trailing slashes
    let path = path.trim_end_matches("/");

    // format the glob pattern
    let glob = format!("{}/*", path);

    // Prepare the query
    // Note that the query includes some of the filtering that was done by the c code. In my experience
    // doing it in sqlite will generally be faster.
    let mut stmt = connection
        .prepare_cached("select key from meta_data where key glob ?1 and key != ?2;")
        .context(EAcess)?;

    // Part of the contract is that we always return these dirs
    cb(".");
    cb("..");

    // Actually do the query
    let mut rows = stmt.query(params![glob, path]).context(EAcess)?;
    while let Some(row) = rows.next().context(EAcess)? {
        // get_ref borrows data from sqlite. This avoids us allocating a new string.
        let row_ref = row
            .get_ref(0)
            .context(EBusy)?
            .as_str()
            .expect("Expected key column to contain text but it is some other type");

        let trimmed_result = &row_ref[path.len() + 1..];

        if !(trimmed_result.is_empty() || trimmed_result.contains("/")) {
            cb(trimmed_result)
        }
    }
    Ok(())
}
