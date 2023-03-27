//! Structures of the ZIP file format.


use std::io::{Read, Write};

use zmx_macros::minimum_length;

use crate::io_ext::{ReadExt, WriteExt};


/// The "End of Central Directory" record.
///
/// This is the only record required by the ZIP file format.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[minimum_length(biased)]
pub(crate) struct EndOfCentralDirectory {
    /// Number of this disk.
    pub disk_no: u16,

    /// Number of the disk with the start of the central directory.
    pub start_central_dir_disk_no: u16,

    /// Total number of entries in the central directory on this disk.
    pub total_central_dir_entries_this_disk: u16,

    /// Total number of entries in the central directory (on all disks).
    pub total_central_dir_entries: u16,

    /// Size of the central directory.
    pub central_directory_size: u32,

    /// Offset of the start of the central directory relative to its disk.
    pub central_dir_offset_on_disk: u32,

    /// The ZIP file comment.
    ///
    /// `None` if there is a comment but it is too long for its size to fit in a 16-bit field. Empty
    /// comments are stored as `Some(v)` with an empty `v`.
    pub comment: Option<Vec<u8>>,
}
impl EndOfCentralDirectory {
    /// The constant signature of an End of Central Directory record.
    ///
    /// It is equivalent to `b"PK\x05\x06"`, interpreted as `u32` in little-endian byte order.
    pub const fn signature() -> u32 { 0x06054B50 }

    const fn min_len_bias() -> u64 {
        4 // signature
    }

    /// Write the end-of-central-directory record.
    pub fn write<W: Write>(&self, mut writer: W) -> Result<(), crate::Error> {
        // write signature
        writer.write_u32_le(Self::signature())?;

        // write out fields in turn
        writer.write_u16_le(self.disk_no)?;
        writer.write_u16_le(self.start_central_dir_disk_no)?;
        writer.write_u16_le(self.total_central_dir_entries_this_disk)?;
        writer.write_u16_le(self.total_central_dir_entries)?;
        writer.write_u32_le(self.central_directory_size)?;
        writer.write_u32_le(self.central_dir_offset_on_disk)?;

        match &self.comment {
            Some(c) => {
                let length: u16 = c.len().try_into().unwrap_or(0xFFFF);
                writer.write_u16_le(length)?;
                writer.write_all(c)?;
            },
            None => {
                writer.write_u16_le(0xFFFF)?;
            },
        }

        Ok(())
    }

    /// Read an end-of-central-directory record.
    pub fn read<R: Read>(&self, mut reader: R) -> Result<Self, crate::Error> {
        let signature_bytes = reader.read_u32_le()?;
        if signature_bytes != Self::signature() {
            return Err(crate::Error::IncorrectSignature);
        }

        let disk_no = reader.read_u16_le()?;
        let start_central_dir_disk_no = reader.read_u16_le()?;
        let total_central_dir_entries_this_disk = reader.read_u16_le()?;
        let total_central_dir_entries = reader.read_u16_le()?;
        let central_directory_size = reader.read_u32_le()?;
        let central_dir_offset_on_disk = reader.read_u32_le()?;

        let comment_length = reader.read_u16_le()?;
        let comment = if comment_length == 0xFFFF {
            None
        } else {
            let buf_length: usize = comment_length.try_into().unwrap();
            let mut buf = vec![0u8; buf_length];
            reader.read_exact(&mut buf)?;
            Some(buf)
        };

        Ok(Self {
            disk_no,
            start_central_dir_disk_no,
            total_central_dir_entries_this_disk,
            total_central_dir_entries,
            central_directory_size,
            central_dir_offset_on_disk,
            comment,
        })
    }
}


/// The "Zip64 End of Central Directory Locator" record.
///
/// This is used to find the [Zip64 End of Central Directory record](Zip64EndOfCentralDirectory). It
/// must be on the same disk as the [End of Central Directory record](EndOfCentralDirectory) and is
/// generally assumed to directly precede it. The Zip64 End of Central Directory record itself may
/// be on a different disk.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[minimum_length(biased)]
pub(crate) struct Zip64EndOfCentralDirectoryLocator {
    /// Number of the disk with the Zip64 End of Central Directory record.
    pub disk_no: u32,

    /// Offset of the Zip64 End of Central Directory record relative to its disk.
    pub offset_on_disk: u64,

    /// The total number of disks in this archive.
    pub total_disks: u32,
}
impl Zip64EndOfCentralDirectoryLocator {
    /// The constant signature of a Zip64 End of Central Directory locator record.
    ///
    /// It is equivalent to `b"PK\x06\x07"`, interpreted as `u32` in little-endian byte order.
    pub const fn signature() -> u32 { 0x07064B50 }

    const fn min_len_bias() -> u64 {
        4 // signature
    }

    /// Write the Zip64 end-of-central-directory locator record.
    pub fn write<W: Write>(&self, mut writer: W) -> Result<(), crate::Error> {
        // write signature
        writer.write_u32_le(Self::signature())?;

        // write out fields in turn
        writer.write_u32_le(self.disk_no)?;
        writer.write_u64_le(self.offset_on_disk)?;
        writer.write_u32_le(self.total_disks)?;

        Ok(())
    }

    /// Read a Zip64 end-of-central-directory locator record.
    pub fn read<R: Read>(&self, mut reader: R) -> Result<Self, crate::Error> {
        let signature_bytes = reader.read_u32_le()?;
        if signature_bytes != Self::signature() {
            return Err(crate::Error::IncorrectSignature);
        }

        let disk_no = reader.read_u32_le()?;
        let offset_on_disk = reader.read_u64_le()?;
        let total_disks = reader.read_u32_le()?;

        Ok(Self {
            disk_no,
            offset_on_disk,
            total_disks,
        })
    }
}


/// The "Zip64 End of Central Directory" record.
///
/// This is used to augment the [End of Central Directory record](EndOfCentralDirectory) with fields
/// with a larger value range, allowing larger files.
#[minimum_length(biased)]
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Zip64EndOfCentralDirectory {
    /// ZIP version supported by the software that created the file.
    pub creator_version: u16,

    /// ZIP version required to extract this ZIP file.
    pub required_version: u16,

    /// Number of this disk.
    pub disk_no: u32,

    /// Number of the disk with the start of the central directory.
    pub start_central_dir_disk_no: u32,

    /// Total number of entries in the central directory on this disk.
    pub total_central_dir_entries_this_disk: u64,

    /// Total number of entries in the central directory (on all disks).
    pub total_central_dir_entries: u64,

    /// Size of the central directory.
    pub central_directory_size: u64,

    /// Offset of the start of the central directory relative to its disk.
    pub central_dir_offset_on_disk: u64,

    /// Zip64 extensible data sector contents.
    pub extensible_data_sector: Vec<u8>,
}
impl Zip64EndOfCentralDirectory {
    /// The constant signature of a Zip64 End of Central Directory record.
    ///
    /// It is equivalent to `b"PK\x06\x06"`, interpreted as `u32` in little-endian byte order.
    pub const fn signature() -> u32 { 0x06064B50 }

    const fn min_len_bias() -> u64 {
        4 // signature
        + 8 // length
    }

    /// Write the Zip64 end-of-central-directory record.
    pub fn write<W: Write>(&self, mut writer: W) -> Result<(), crate::Error> {
        // length is that of the whole structure including the extensible data sector
        // but excluding the signature (4 bytes) and the length field (8 bytes)
        let length: u64 = Self::min_len() + u64::try_from(self.extensible_data_sector.len()).unwrap();

        // write signature and length
        writer.write_u32_le(Self::signature())?;
        writer.write_u64_le(length)?;

        // write out fields in turn
        writer.write_u16_le(self.creator_version)?;
        writer.write_u16_le(self.required_version)?;
        writer.write_u32_le(self.disk_no)?;
        writer.write_u32_le(self.start_central_dir_disk_no)?;
        writer.write_u64_le(self.total_central_dir_entries_this_disk)?;
        writer.write_u64_le(self.total_central_dir_entries)?;
        writer.write_u64_le(self.central_directory_size)?;
        writer.write_u64_le(self.central_dir_offset_on_disk)?;
        writer.write_all(&self.extensible_data_sector)?;

        Ok(())
    }

    /// Read a Zip64 end-of-central-directory locator record.
    pub fn read<R: Read>(&self, mut reader: R) -> Result<Self, crate::Error> {
        let signature_bytes = reader.read_u32_le()?;
        if signature_bytes != Self::signature() {
            return Err(crate::Error::IncorrectSignature);
        }

        let size = reader.read_u64_le()?;
        const FIXED_FIELDS_LEN: u64 = Zip64EndOfCentralDirectory::min_len() - Zip64EndOfCentralDirectory::min_len_bias();
        if size < FIXED_FIELDS_LEN {
            return Err(crate::Error::RecordTooSmall);
        }
        let extensible_length: usize = (size - Self::min_len()).try_into().unwrap();

        let creator_version = reader.read_u16_le()?;
        let required_version = reader.read_u16_le()?;
        let disk_no = reader.read_u32_le()?;
        let start_central_dir_disk_no = reader.read_u32_le()?;
        let total_central_dir_entries_this_disk = reader.read_u64_le()?;
        let total_central_dir_entries = reader.read_u64_le()?;
        let central_directory_size = reader.read_u64_le()?;
        let central_dir_offset_on_disk = reader.read_u64_le()?;

        let mut extensible_data_sector = vec![0u8; extensible_length];
        reader.read_exact(&mut extensible_data_sector)?;

        Ok(Self {
            creator_version,
            required_version,
            disk_no,
            start_central_dir_disk_no,
            total_central_dir_entries_this_disk,
            total_central_dir_entries,
            central_directory_size,
            central_dir_offset_on_disk,
            extensible_data_sector,
        })
    }
}


/// The "Central Directory Header" record.
///
/// This contains information about a single directory entry.
#[minimum_length(biased)]
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct CentralDirectoryHeader {
    /// ZIP version supported by the software that created this entry.
    pub creator_version: u16,

    /// ZIP version required to extract this entry.
    pub required_version: u16,

    /// General-purpose field of bit flags.
    pub general_purpose_bit_flag: u16,

    /// Method with which the file was compressed.
    pub compression_method: u32,

    /// The file's time of last modification.
    pub last_mod_file_time: u16,

    /// The file's date of last modification.
    pub last_mod_file_date: u16,

    /// CRC-32 checksum of the data.
    pub crc32: u32,

    /// The compressed size of this file.
    pub compressed_size: u32,

    /// The uncompressed size of this file.
    pub uncompressed_size: u32,

    /// The file name of this entry.
    pub file_name: Vec<u8>,

    /// Data in the extra field of this entry.
    pub extra_field: Vec<u8>,

    // todo:
    // * file comment (length; data at end)
    // * disk number start
    // * internal file attributes
    // * external file attributes
    // * relative offset of local header
}
impl CentralDirectoryHeader {
    /// The constant signature of a Central Directory Header record.
    ///
    /// It is equivalent to `b"PK\x01\x02"`, interpreted as `u32` in little-endian byte order.
    pub const fn signature() -> u32 { 0x02014B50 }

    const fn min_len_bias() -> u64 {
        4 // signature
    }

    /// Write the central directory header record.
    pub fn write<W: Write>(&self, mut writer: W) -> Result<(), crate::Error> {
        // write signature
        writer.write_u32_le(Self::signature())?;

        let file_name_length: u16 = if self.file_name.len() > 0xFFFF {
            0xFFFF
        } else {
            self.file_name.len().try_into().unwrap()
        };
        let extra_field_length: u16 = if self.extra_field.len() > 0xFFFF {
            0xFFFF
        } else {
            self.extra_field.len().try_into().unwrap()
        };

        // write out fields in turn
        writer.write_u16_le(self.creator_version)?;
        writer.write_u16_le(self.required_version)?;
        writer.write_u16_le(self.general_purpose_bit_flag)?;
        writer.write_u32_le(self.compression_method)?;
        writer.write_u16_le(self.last_mod_file_time)?;
        writer.write_u16_le(self.last_mod_file_date)?;
        writer.write_u32_le(self.crc32)?;
        writer.write_u32_le(self.compressed_size)?;
        writer.write_u32_le(self.uncompressed_size)?;
        writer.write_u16_le(file_name_length)?;
        writer.write_u16_le(extra_field_length)?;

        writer.write_all(&self.file_name)?;
        writer.write_all(&self.extra_field)?;

        Ok(())
    }

    /// Read a Zip64 end-of-central-directory locator record.
    pub fn read<R: Read>(&self, mut reader: R) -> Result<Self, crate::Error> {
        let signature_bytes = reader.read_u32_le()?;
        if signature_bytes != Self::signature() {
            return Err(crate::Error::IncorrectSignature);
        }

        let size = reader.read_u64_le()?;
        const FIXED_FIELDS_LEN: u64 = Zip64EndOfCentralDirectory::min_len() - Zip64EndOfCentralDirectory::min_len_bias();
        if size < FIXED_FIELDS_LEN {
            return Err(crate::Error::RecordTooSmall);
        }

        todo!();
    }
}
