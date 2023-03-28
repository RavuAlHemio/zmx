use std::env;
use std::ffi::OsString;
use std::fs::File;

use libzmx::{best_effort_decode, zip_get_files};


fn main() {
    let args: Vec<OsString> = env::args_os()
        .collect();
    let mut zip_file = File::open(&args[1])
        .expect("failed to open ZIP file");
    let entries = zip_get_files(&mut zip_file)
        .expect("failed to get files from ZIP file");
    for cde in &entries {
        let name_string = best_effort_decode(&cde.entry.file_name);
        println!(
            "# {}: {}\n{:?}",
            cde.offset, name_string, cde,
        );
        println!(
            "crt=0x{:04X} req=0x{:04X} gpbf=0x{:04X}, inta=0x{:04X}, exta=0x{:04X}",
            cde.entry.creator_version, cde.entry.required_version,
            cde.entry.general_purpose_bit_flag, cde.entry.internal_attributes,
            cde.entry.external_attributes,
        );
        println!();
    }
}
