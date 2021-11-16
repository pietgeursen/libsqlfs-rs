# libsqlfs-rs

> Beginnings of porting libsqlfs to rust by slices

## Building

- `cargo build --release`
- `cp target/release/libsqlfs_rs.a <libsqlfs-dir>/lib`
- `cbindgen > libsqlfs_rs.h`
- `cp libsqlfs_rs.h <libsqlfs-dir>`

In the libsqlfs directory
- `make`

## Running
In the libsqlfs directory

- Mount a dir using fuse
  - `mkdir my_sqlite_fuse_dir`
  - `./fuse_sqlfs my_sqlite_fuse_dir`
- Put something in your dir
  - `touch my_sqlite_fuse_dir/my_file.md`
- List the files (will call our rust code)
  - `./sqlfsls /tmp/fsdata`
