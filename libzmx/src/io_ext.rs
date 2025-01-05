//! Input/output extensions for reading and writing binary data.


use std::io;


macro_rules! implement_read {
    ($be_name:ident, $le_name:ident, $int_ty:ident, $byte_count:literal) => {
        #[allow(unused)]
        #[inline]
        fn $be_name(&mut self) -> Result<$int_ty, ::std::io::Error> {
            let mut bytes = [0u8; $byte_count];
            self.read_exact(&mut bytes)?;
            Ok($int_ty::from_be_bytes(bytes))
        }

        #[allow(unused)]
        #[inline]
        fn $le_name(&mut self) -> Result<$int_ty, ::std::io::Error> {
            let mut bytes = [0u8; $byte_count];
            self.read_exact(&mut bytes)?;
            Ok($int_ty::from_le_bytes(bytes))
        }
    };
}
macro_rules! implement_read_signed {
    ($signed_ty:ident, $signed_name:ident, $unsigned_name:ident, $comment:expr) => {
        #[doc = $comment]
        #[allow(unused)]
        #[inline]
        #[must_use]
        fn $signed_name(&mut self) -> Result<$signed_ty, ::std::io::Error> {
            Ok(self.$unsigned_name()? as $signed_ty)
        }
    };
}

macro_rules! implement_write {
    ($be_name:ident, $le_name:ident, $int_ty:ident, $byte_count:literal) => {
        #[allow(unused)]
        #[inline]
        fn $be_name(&mut self, val: $int_ty) -> Result<(), ::std::io::Error> {
            let bytes: [u8; $byte_count] = val.to_be_bytes();
            self.write_all(&bytes)
        }

        #[allow(unused)]
        #[inline]
        fn $le_name(&mut self, val: $int_ty) -> Result<(), ::std::io::Error> {
            let bytes: [u8; $byte_count] = val.to_le_bytes();
            self.write_all(&bytes)
        }
    };
}
macro_rules! implement_write_signed {
    ($signed_ty:ident, $signed_name:ident, $unsigned_ty:ident, $unsigned_name:ident, $comment:expr) => {
        #[doc = $comment]
        #[allow(unused)]
        #[inline]
        #[must_use]
        fn $signed_name(&mut self, value: $signed_ty) -> Result<(), ::std::io::Error> {
            self.$unsigned_name(value as $unsigned_ty)
        }
    };
}

/// Extensions for reading binary data.
pub(crate) trait ReadExt {
    #[doc = "Read an unsigned 8-bit integer."] #[must_use] fn read_u8(&mut self) -> Result<u8, io::Error>;

    #[doc = "Read an unsigned 16-bit integer in little-endian byte order."] #[must_use] fn read_u16_le(&mut self) -> Result<u16, io::Error>;
    #[doc = "Read an unsigned 16-bit integer in big-endian byte order."] #[must_use] fn read_u16_be(&mut self) -> Result<u16, io::Error>;

    #[doc = "Read an unsigned 32-bit integer in little-endian byte order."] #[must_use] fn read_u32_le(&mut self) -> Result<u32, io::Error>;
    #[doc = "Read an unsigned 32-bit integer in big-endian byte order."] #[must_use] fn read_u32_be(&mut self) -> Result<u32, io::Error>;

    #[doc = "Read an unsigned 64-bit integer in little-endian byte order."] #[must_use] fn read_u64_le(&mut self) -> Result<u64, io::Error>;
    #[doc = "Read an unsigned 64-bit integer in big-endian byte order."] #[must_use] fn read_u64_be(&mut self) -> Result<u64, io::Error>;

    #[doc = "Read an unsigned 128-bit integer in little-endian byte order."] #[must_use] fn read_u128_le(&mut self) -> Result<u128, io::Error>;
    #[doc = "Read an unsigned 128-bit integer in big-endian byte order."] #[must_use] fn read_u128_be(&mut self) -> Result<u128, io::Error>;

    implement_read_signed!(i8, read_i8, read_u8, "Read a signed 8-bit integer.");

    implement_read_signed!(i16, read_i16_le, read_u16_le, "Read a signed 16-bit integer in little-endian byte order.");
    implement_read_signed!(i16, read_i16_be, read_u16_be, "Read a signed 16-bit integer in big-endian byte order.");

    implement_read_signed!(i32, read_i32_le, read_u32_le, "Read a signed 32-bit integer in little-endian byte order.");
    implement_read_signed!(i32, read_i32_be, read_u32_be, "Read a signed 32-bit integer in big-endian byte order.");

    implement_read_signed!(i64, read_i64_le, read_u64_le, "Read a signed 64-bit integer in little-endian byte order.");
    implement_read_signed!(i64, read_i64_be, read_u64_be, "Read a signed 64-bit integer in big-endian byte order.");

    implement_read_signed!(i128, read_i128_le, read_u128_le, "Read a signed 128-bit integer in little-endian byte order.");
    implement_read_signed!(i128, read_i128_be, read_u128_be, "Read a signed 128-bit integer in big-endian byte order.");
}
impl<R: io::Read> ReadExt for R {
    #[inline]
    fn read_u8(&mut self) -> Result<u8, io::Error> {
        let mut buf = [0];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    implement_read!(read_u16_be, read_u16_le, u16, 2);
    implement_read!(read_u32_be, read_u32_le, u32, 4);
    implement_read!(read_u64_be, read_u64_le, u64, 8);
    implement_read!(read_u128_be, read_u128_le, u128, 16);
}

/// Extensions for writing binary data.
pub(crate) trait WriteExt {
    #[doc = "Write an unsigned 8-bit integer."] #[must_use] fn write_u8(&mut self, val: u8) -> Result<(), io::Error>;

    #[doc = "Write an unsigned 16-bit integer in little-endian byte order."] #[must_use] fn write_u16_le(&mut self, val: u16) -> Result<(), io::Error>;
    #[doc = "Write an unsigned 16-bit integer in big-endian byte order."] #[must_use] fn write_u16_be(&mut self, val: u16) -> Result<(), io::Error>;

    #[doc = "Write an unsigned 32-bit integer in little-endian byte order."] #[must_use] fn write_u32_le(&mut self, val: u32) -> Result<(), io::Error>;
    #[doc = "Write an unsigned 32-bit integer in big-endian byte order."] #[must_use] fn write_u32_be(&mut self, val: u32) -> Result<(), io::Error>;

    #[doc = "Write an unsigned 64-bit integer in little-endian byte order."] #[must_use] fn write_u64_le(&mut self, val: u64) -> Result<(), io::Error>;
    #[doc = "Write an unsigned 64-bit integer in big-endian byte order."] #[must_use] fn write_u64_be(&mut self, val: u64) -> Result<(), io::Error>;

    #[doc = "Write an unsigned 128-bit integer in little-endian byte order."] #[must_use] fn write_u128_le(&mut self, val: u128) -> Result<(), io::Error>;
    #[doc = "Write an unsigned 128-bit integer in big-endian byte order."] #[must_use] fn write_u128_be(&mut self, val: u128) -> Result<(), io::Error>;

    implement_write_signed!(i8, write_i8, u8, write_u8, "Write a signed 8-bit integer.");

    implement_write_signed!(i16, write_i16_le, u16, write_u16_le, "Write a signed 16-bit integer in little-endian byte order.");
    implement_write_signed!(i16, write_i16_be, u16, write_u16_be, "Write a signed 16-bit integer in big-endian byte order.");

    implement_write_signed!(i32, write_i32_le, u32, write_u32_le, "Write a signed 32-bit integer in little-endian byte order.");
    implement_write_signed!(i32, write_i32_be, u32, write_u32_be, "Write a signed 32-bit integer in big-endian byte order.");

    implement_write_signed!(i64, write_i64_le, u64, write_u64_le, "Write a signed 64-bit integer in little-endian byte order.");
    implement_write_signed!(i64, write_i64_be, u64, write_u64_be, "Write a signed 64-bit integer in big-endian byte order.");

    implement_write_signed!(i128, write_i128_le, u128, write_u128_le, "Write a signed 128-bit integer in little-endian byte order.");
    implement_write_signed!(i128, write_i128_be, u128, write_u128_be, "Write a signed 128-bit integer in big-endian byte order.");
}
impl<W: io::Write> WriteExt for W {
    #[inline]
    fn write_u8(&mut self, val: u8) -> Result<(), io::Error> {
        self.write_all(&[val])
    }

    implement_write!(write_u16_be, write_u16_le, u16, 2);
    implement_write!(write_u32_be, write_u32_le, u32, 4);
    implement_write!(write_u64_be, write_u64_le, u64, 8);
    implement_write!(write_u128_be, write_u128_le, u128, 16);
}
