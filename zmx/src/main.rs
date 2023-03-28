use std::env;
use std::ffi::OsString;
use std::fs::File;

use libzmx::zip_get_files;


fn main() {
    let args: Vec<OsString> = env::args_os()
        .collect();
    let mut zip_file = File::open(&args[1])
        .expect("failed to open ZIP file");
    zip_get_files(&mut zip_file)
        .expect("failed to get files from ZIP file");
}
