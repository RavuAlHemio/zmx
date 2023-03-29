use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use libzmx::{best_effort_decode, ZipCentralDirectoryEntry, zip_get_files, zip_make_executable};


#[derive(Parser)]
struct Opts {
    /// The path to the ZIP file to modify.
    pub zip_path: PathBuf,

    /// The names of the ZIP entries to make executable.
    pub executable_files: Vec<Vec<u8>>,
}


fn main() -> ExitCode {
    let opts = Opts::parse();

    {
        let mut zip_file = File::options()
            .read(true)
            .write(true)
            .append(false)
            .truncate(false)
            .open(&opts.zip_path)
            .expect("failed to open ZIP file");

        // collect entry names
        let entries = zip_get_files(&mut zip_file)
            .expect("failed to get file list from ZIP file");
        let name_to_entry: HashMap<&[u8], &ZipCentralDirectoryEntry> = entries
            .iter()
            .map(|e| (e.entry.file_name.as_slice(), e))
            .collect();

        let mut bad = false;
        for exec_file in &opts.executable_files {
            if !name_to_entry.contains_key(exec_file.as_slice()) {
                let entry_name = best_effort_decode(exec_file);
                eprintln!("ZIP file {} does not contain entry {:?}", opts.zip_path.display(), entry_name);
                bad = true;
            }
        }
        if bad {
            return ExitCode::FAILURE;
        }

        // make requested files executable
        // store locations in BTree map to make sure we mostly seek forward
        let mut exec_location_to_path: BTreeMap<u64, String> = BTreeMap::new();
        for exec_file in &opts.executable_files {
            let entry = name_to_entry.get(exec_file.as_slice())
                .expect("entry suddenly disappeared from central directory");
            let entry_name = best_effort_decode(exec_file.as_slice());
            exec_location_to_path.insert(entry.offset, entry_name);
        }

        for (exec_location, path) in exec_location_to_path {
            if let Err(e) = zip_make_executable(&mut zip_file, exec_location) {
                panic!("failed to make {:?} executable: {}", path, e);
            }
        }
    }

    ExitCode::SUCCESS
}
