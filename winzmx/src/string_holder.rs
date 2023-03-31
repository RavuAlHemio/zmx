use std::mem::size_of;

use windows::core::PCWSTR;


#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StringHolder {
    words: Vec<u16>,
}
impl StringHolder {
    /// Creates a new StringHolder with the value of the given string slice.
    pub fn from_str(s: &str) -> Self {
        let mut words: Vec<u16> = s.encode_utf16().collect();
        words.push(0x0000);
        Self {
            words,
        }
    }

    /// Creates a new StringHolder by copying u16s from the given pointer until a NUL u16 is
    /// encountered or, if a maximum length is given, until this number of u16s is copied.
    pub fn from_ptr_nul_terminated(mut ptr: *const u16, max_length: Option<usize>) -> Self {
        let mut words = Vec::new();
        loop {
            let word = unsafe { *ptr };
            words.push(word);
            if word == 0x0000 {
                break;
            }
            if let Some(ml) = max_length {
                if words.len() == ml {
                    // (otherwise we'd have broken out earlier)
                    assert_ne!(word, 0x0000);

                    // ensure NUL termination
                    words.push(0x0000);
                    break;
                }
            }
            ptr = unsafe { ptr.offset(1) };
        }
        Self {
            words,
        }
    }

    /// Creates a new StringHolder by copying the given number of u16s from the given pointer.
    pub fn from_ptr_with_length(ptr: *const u16, length: usize) -> Self {
        let slice = unsafe { std::slice::from_raw_parts(ptr, length) };
        let words = Vec::from(slice);
        Self {
            words,
        }
    }

    /// Creates a new StringHolder by copying u16s from the given slice until a NUL u16 is
    /// encountered or the end of the slice is reached.
    pub fn from_slice_nul_terminated(slice: &[u16]) -> Self {
        let mut words = Vec::with_capacity(slice.len()+1);
        for &word in slice {
            words.push(word);
            if word == 0x0000 {
                return Self {
                    words,
                };
            }
        }
        // we reached the end of the slice without encountering a NUL
        // append it before we return
        words.push(0x0000);
        Self {
            words,
        }
    }

    /// The length of the string in this StringHolder, in units of u16s.
    ///
    /// Depending on the argument, counts the terminating NUL character or not.
    #[inline]
    pub fn len_u16s(&self, count_nul: bool) -> usize {
        if count_nul {
            self.words.len()
        } else {
            self.words.len() - 1
        }
    }

    /// The length of the string in this StringHolder, in units of bytes.
    ///
    /// Depending on the argument, counts the terminating NUL character or not.
    #[inline]
    pub fn len_bytes(&self, count_nul: bool) -> usize {
        self.len_u16s(count_nul) * size_of::<u16>()
    }

    /// Returns a pointer to the u16s backing this StringHolder.
    #[inline]
    pub fn as_ptr(&self) -> *const u16 {
        self.words.as_ptr()
    }

    /// Returns a pointer to the u16s backing this StringHolder as a [PCWSTR].
    #[inline]
    pub fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.as_ptr())
    }

    /// Attempt to decode the held string as UTF-16.
    ///
    /// Returns `None` if the held string is not valid UTF-16.
    pub fn try_to_string(&self) -> Option<String> {
        String::from_utf16(self.as_slice(false)).ok()
    }

    /// The string as a slice of u16s, with or without the terminating NUL.
    #[inline]
    pub fn as_slice(&self, include_nul: bool) -> &[u16] {
        if include_nul {
            &self.words[0..self.words.len()]
        } else {
            &self.words[0..self.words.len()-1]
        }
    }
}
