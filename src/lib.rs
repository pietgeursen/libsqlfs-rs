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
    EBusy,
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
    handle: *mut c_void,
    path_ptr: *const c_char,
    filler_buff: *mut c_void,
    filler: fuse_fill_dir_t,
) -> c_int {
    // Convert the sqlite3 pointer to a Connection.
    let connection = unsafe { Connection::from_handle(handle as *mut sqlite3) }
        .expect("todo: couldn't open Connection from handle");

    // Convert the path_ptr to a rust &str
    let path = unsafe { CStr::from_ptr(path_ptr) }
        .to_str()
        .expect("todo: couldn't convert path to valid utf8 rust str");

    // Run our query
    let result = readdir_(connection, path, |st| {
        let s = CString::new(st).unwrap();

        if let Some(filler) = filler {
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

    // Actually do the query
    let mut entries_iter = stmt
        .query_map(params![glob, path], |row| {
            let row_ref = row.get_ref(0)?.as_str()?;

            let trimmed_result: String = (&row_ref[path.len() + 1..]).into();
            Ok(trimmed_result)
        })
        .context(EAcess)?
        .filter(|val| {
            // Some results need to be filtered out
            match val {
                // Skip if grandchild etc
                Ok(val) => !(val.is_empty() || val.contains("/")),
                _ => true,
            }
        });

    // Part of the contract is that we always return these dirs
    cb(".");
    cb("..");

    // If any loop returns an Err then we return that error immediately using `try_for_each`.
    // This is slightly different from the c code. If any one result returns busy it will keep
    // trying the next row. Which seems weird to me?
    entries_iter.try_for_each(|entry| {
        match entry {
            Ok(entry) => Ok(cb(&entry)),
            // We can't distinguish between SQLITE_BUSY or some other sqlite error because they're
            // not exposed in this error type. We could probably get at them if we really cared.
            // This just catches all errors and return EBusy
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
