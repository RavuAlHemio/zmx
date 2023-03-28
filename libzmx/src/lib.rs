//! # ZIP Make eXecutable
//!
//! Allows dynamic modification of ZIP archives to set some files as executable (by changing their
//! origin to Unix and setting their external file attributes).


mod io_ext;
mod zip_format;


use std::fmt;
use std::io::{Read, Seek, SeekFrom};

use io_ext::ReadExt;

use crate::zip_format::EndOfCentralDirectory;


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

    todo!();
}
