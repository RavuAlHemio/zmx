//! # ZIP Make eXecutable
//!
//! Allows dynamic modification of ZIP archives to set some files as executable (by changing their
//! origin to Unix and setting their external file attributes).


mod io_ext;
mod zip_format;


use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};

use crate::io_ext::{ReadExt, WriteExt};
use crate::zip_format::{
    CentralDirectoryEntry, EndOfCentralDirectory, Zip64EndOfCentralDirectory,
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


/// An entry encountered in a ZIP archive's central directory. Represents a single file system item
/// (file, folder, etc.).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ZipCentralDirectoryEntry {
    /// The actual information about this entry.
    pub entry: CentralDirectoryEntry,

    /// The number of the disk containing this central directory entry.
    pub disk: u32,

    /// The offset of this file's central directory entry from the beginning of its disk.
    pub offset: u64,
}
impl ZipCentralDirectoryEntry {
    /// Returns whether this entry is executable.
    ///
    /// An entry is considered executable if all of the following conditions are met:
    ///
    /// * According to the DOS file attributes, the entry is not a directory. (In the lower half of
    ///   the "external file attributes" field, the bit corresponding to the value 0x10 is not set.)
    /// * The file has been created on a Unix system. (The upper byte of the "version made by" field
    ///   is 0x03.)
    /// * According to the Unix file attributes, the entry is a regular file. (In the top half of
    ///   the "external file attributes" field, the bits extracted using the mask 0o170000 are
    ///   0o100000.)
    /// * According to the Unix file attributes, at least user or group or others have permission
    ///   to execute the file. (In the top half of the "external file attributes" field, the bits
    ///   extracted using the mask 0o000111 are not 0o000000.)
    pub const fn is_executable(&self) -> bool {
        let dos_attribs = (self.entry.external_attributes >> 0) & 0x0000FFFF;
        if dos_attribs & 0x10 != 0 {
            // it's a directory!
            return false;
        }

        if ((self.entry.creator_version >> 8) & 0xFF) != 0x03 {
            // entry does not come from Unix
            return false;
        }

        let unix_attribs = (self.entry.external_attributes >> 16) & 0x0000FFFF;
        if unix_attribs & 0o170000 != 0o100000 {
            // not a regular file
            return false;
        }
        // return whether at least u/g/o has x
        unix_attribs & 0o000111 != 0o000000
    }
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

/// Attempts to decode the given byte slice as UTF-8; if this fails, stubbornly decodes it as
/// ISO-8859-1 instead.
pub fn best_effort_decode(bytes: &[u8]) -> String {
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
pub fn zip_get_files<F: Read + Seek>(mut zip_file: F) -> Result<Vec<ZipCentralDirectoryEntry>, Error> {
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
        if signature != CentralDirectoryEntry::signature() {
            break;
        }
        let cdh = CentralDirectoryEntry::read_after_signature(&mut zip_file)?;
        file_names.push(ZipCentralDirectoryEntry {
            entry: cdh,
            disk: 0,
            offset: file_header_loc,
        });
    }

    Ok(file_names)
}


/// Modifies the attributes of a ZIP file entry to make it executable.
pub fn zip_make_executable<F: Read + Seek + Write>(mut zip_file: F, entry_header_offset: u64) -> Result<(), Error> {
    // seek to the given offset
    zip_file.seek(SeekFrom::Start(entry_header_offset))?;

    // check for central directory entry
    let signature = zip_file.read_u32_le()?;
    if signature != CentralDirectoryEntry::signature() {
        return Err(Error::IncorrectSignature);
    }

    // set upper byte of creator version to 0x03 (Unix)
    let mut creator_version = zip_file.read_u16_le()?;
    creator_version = (creator_version & 0x00FF) | 0x0300;
    zip_file.seek(SeekFrom::Current(-2))?;
    zip_file.write_u16_le(creator_version)?;

    // skip the intervening fields
    zip_file.seek(SeekFrom::Current(
        2 // required_version
        + 2 // general_purpose_bit_flag
        + 2 // compression_method
        + 2 // last_mod_file_time
        + 2 // last_mod_file_date
        + 4 // crc32
        + 4 // compressed_size
        + 4 // uncompressed_size
        + 2 // file_name length
        + 2 // extra_fields length
        + 2 // file_comment length
        + 2 // disk_number_start
        + 2 // internal_attributes
    ))?;

    // perform this change to upper byte pair of external attributes:
    // 1. ensure bytes 0o170000 are set to 0o100000
    // 2. ensure bits 0o000111 are set
    let mut external_attributes = zip_file.read_u32_le()?;
    external_attributes =
        (external_attributes & ((0o170000 << 16) ^ 0xFFFF_FFFF))
        | (0o100000 << 16)
    ;
    external_attributes |= 0o000111 << 16;
    zip_file.seek(SeekFrom::Current(-4))?;
    zip_file.write_u32_le(external_attributes)?;

    // done
    Ok(())
}


/// Modifies the attributes of a ZIP file entry to make it not executable.
pub fn zip_make_not_executable<F: Read + Seek + Write>(mut zip_file: F, entry_header_offset: u64) -> Result<(), Error> {
    // seek to the given offset
    zip_file.seek(SeekFrom::Start(entry_header_offset))?;

    // check for central directory entry
    let signature = zip_file.read_u32_le()?;
    if signature != CentralDirectoryEntry::signature() {
        return Err(Error::IncorrectSignature);
    }

    // check upper byte of creator version against 0x03 (Unix)
    let creator_version = zip_file.read_u16_le()?;
    if (creator_version & 0xFF00) != 0x0300 {
        // not Unix, cannot be executable
        return Ok(());
    }

    // skip the intervening fields
    zip_file.seek(SeekFrom::Current(
        2 // required_version
        + 2 // general_purpose_bit_flag
        + 2 // compression_method
        + 2 // last_mod_file_time
        + 2 // last_mod_file_date
        + 4 // crc32
        + 4 // compressed_size
        + 4 // uncompressed_size
        + 2 // file_name length
        + 2 // extra_fields length
        + 2 // file_comment length
        + 2 // disk_number_start
        + 2 // internal_attributes
    ))?;

    // remove 0o000111 from upper byte pair of external attributes (if necessary)
    let mut external_attributes = zip_file.read_u32_le()?;
    if (external_attributes & (0o000111 << 16)) != 0 {
        external_attributes &= !(0o000111 << 16);
        zip_file.seek(SeekFrom::Current(-4))?;
        zip_file.write_u32_le(external_attributes)?;
    }

    // done
    Ok(())
}
