//! # ZIP Make eXecutable
//!
//! Allows dynamic modification of ZIP archives to set some files as executable (by changing their
//! origin to Unix and setting their external file attributes).


mod io_ext;
mod zip_format;


use std::fmt;
use std::io::{Read, Seek, SeekFrom};

use io_ext::ReadExt;

use crate::zip_format::{
    CentralDirectoryHeader, EndOfCentralDirectory, Zip64EndOfCentralDirectory,
    Zip64EndOfCentralDirectoryLocator,
};


/// An error that may occur during ZIP decoding or encoding.
#[derive(Debug)]
pub enum Error {
    /// An input/output error.
    Io(std::io::Error),

    /// Missing end-of-central-directory record.
    MissingEndOfCentralDirectory,

    /// A ZIP archive spanning multiple disks/files is being read.
    ///
    /// Spanned ZIP archives are currently not supported.
    SpannedArchive,

    /// A field is too long to be read/written.
    FieldTooLong,

    /// An incorrect signature for the given structure was found.
    IncorrectSignature,

    /// A record is smaller than its minimum size.
    RecordTooSmall,

    /// The extra data has an unexpected length.
    ///
    /// The contained value can be used to seek to the next extra data entry.
    UnexpectedExtraDataLength(u16),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e)
                => write!(f, "I/O error: {}", e),
            Self::MissingEndOfCentralDirectory
                => write!(f, "missing end-of-central-directory record"),
            Self::SpannedArchive
                => write!(f, "ZIP archive spans multiple files/disks"),
            Self::FieldTooLong
                => write!(f, "field too long"),
            Self::IncorrectSignature
                => write!(f, "incorrect signature for structure"),
            Self::RecordTooSmall
                => write!(f, "record too small"),
            Self::UnexpectedExtraDataLength(_)
                => write!(f, "unexpected length of extra data"),
        }
    }
}
impl std::error::Error for Error {
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self { Self::Io(value) }
}


fn lookback_for_signature<F: Read + Seek>(mut file: F, signature: u32) -> Result<bool, Error> {
    loop {
        let possible_signature = file.read_u32_le()?;
        if possible_signature == signature {
            return Ok(true);
        }
        let new_loc = file.seek(SeekFrom::Current(-5))?;
        if new_loc == 0 {
            return Ok(false);
        }
    }
}

fn best_effort_decode(bytes: &[u8]) -> String {
    match String::from_utf8(Vec::from(bytes)) {
        Ok(s) => s,
        Err(_) => {
            bytes.iter()
                .map(|b| char::from_u32(*b as u32).unwrap())
                .collect()
        },
    }
}


/// Obtains the list of file names in the archive.
pub fn zip_get_files<F: Read + Seek>(mut zip_file: F) -> Result<Vec<Vec<u8>>, Error> {
    // start at the last possible location of the End of Central Directory record
    let eocd_start = -i64::try_from(EndOfCentralDirectory::min_len()).unwrap();
    zip_file.seek(SeekFrom::End(eocd_start))?;

    // look for EoCD
    let eocd_found = lookback_for_signature(&mut zip_file, EndOfCentralDirectory::signature())?;
    if !eocd_found {
        return Err(Error::MissingEndOfCentralDirectory);
    }

    // read EoCD
    let eocd = EndOfCentralDirectory::read_after_signature(&mut zip_file)?;
    if eocd.disk_no != 0 {
        return Err(Error::SpannedArchive);
    }
    let mut zip64_central_directory_loc: Option<u64> = None;
    if eocd.should_check_zip64() {
        // go back to EoCD start
        lookback_for_signature(&mut zip_file, EndOfCentralDirectory::signature())?;

        // try to find Zip64 EoCD locator
        let zip64_eocd_loc_found = lookback_for_signature(&mut zip_file, Zip64EndOfCentralDirectoryLocator::signature())?;
        if zip64_eocd_loc_found {
            let zip64_eocd_loc = Zip64EndOfCentralDirectoryLocator::read_after_signature(&mut zip_file)?;
            if zip64_eocd_loc.disk_no != 0 || zip64_eocd_loc.total_disks != 1 {
                return Err(Error::SpannedArchive);
            }

            // try to find Zip64 EoCD
            zip_file.seek(SeekFrom::Start(zip64_eocd_loc.offset_on_disk))?;

            // try to read Zip64 EoCD
            let zip64_eocd_sig = zip_file.read_u32_le()?;
            if zip64_eocd_sig == Zip64EndOfCentralDirectory::signature() {
                let zip64_eocd = Zip64EndOfCentralDirectory::read_after_signature(&mut zip_file)?;
                if zip64_eocd.total_central_dir_entries != zip64_eocd.total_central_dir_entries_this_disk {
                    return Err(Error::SpannedArchive);
                }
                zip64_central_directory_loc = Some(zip64_eocd.central_dir_offset_on_disk);
            }
        }
    }
    let central_directory_loc: u64 = if let Some(zcdl) = zip64_central_directory_loc {
        zcdl
    } else {
        if eocd.total_central_dir_entries != eocd.total_central_dir_entries_this_disk {
            return Err(Error::SpannedArchive);
        }
        eocd.central_dir_offset_on_disk.into()
    };
    zip_file.seek(SeekFrom::Start(central_directory_loc))?;

    // now we can read out the files
    let mut file_names = Vec::new();
    loop {
        let file_header_loc = zip_file.seek(SeekFrom::Current(0))?;
        let signature = zip_file.read_u32_le()?;
        if signature != CentralDirectoryHeader::signature() {
            break;
        }
        let cdh = CentralDirectoryHeader::read_after_signature(&mut zip_file)?;
        let file_name = best_effort_decode(&cdh.file_name);
        println!("{:?} ({})", file_name, cdh.uncompressed_size);
        file_names.push(cdh.file_name.clone());
    }

    Ok(file_names)
}
