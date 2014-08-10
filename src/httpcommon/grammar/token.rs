//! Things pertaining to the RFC 7230 `token` grammar rule.
//!
//! RFC 7230 grammar:
//!
//! ```abnf
//! token          = 1*tchar
//!
//! tchar          = "!" / "#" / "$" / "%" / "&" / "'" / "*"
//!                / "+" / "-" / "." / "^" / "_" / "`" / "|" / "~"
//!                / DIGIT / ALPHA
//!                ; any VCHAR, except delimiters
//! ```
//!
//! Possibly of interest is that RFC 2616 actually defined things the other way
//! round, with the primary definition being that of what is now labelled the
//! delimiters but which was at that time known as *separators*:
//!
//! ```bnf
//! [REMEMBER THIS RULE IS OBSOLETE!]
//! separators: "(" | ")" | "<" | ">" | "@" | "," | ";" | ":"
//!           | "\" | <"> | "/" | "[" | "]" | "?" | "=" | "{"
//!           | "}" | SP | HT
//! ```

use std::fmt;
use std::str;

/// tchar: a token character; any VCHAR, except delimiters.
#[inline]
pub fn is_tchar(o: u8) -> bool {
    o == b'!' || o == b'#' || o == b'$' || o == b'%' || o == b'&' || o == b'\'' ||
    o == b'*' || o == b'+' || o == b'-' || o == b'.' || o == b'^' || o == b'_' ||
    o == b'`' || o == b'|' || o == b'~' || (o >= b'0' && o <= b'9') ||
    (o >= b'A' && o <= b'Z') || (o >= b'a' && o <= b'z')
}

/// A type representing an RFC 7230 `token`.
///
/// This permits strict character set control in a way that a simple `Vec<u8>`
/// or `String` would not.
///
/// This may be either owned, corresponding to `String`/`Vec<u8>`, or a slice,
/// corresponding to `&str`/`&[u8]`.
#[deriving(Clone, Hash)]
pub enum Token<'a> {
    /// A token backed by a vector (`Vec<u8>`).
    #[doc(hidden)]
    Owned {
        #[doc(hidden)]
        pub _bytes: Vec<u8>,
    },
    /// A token backed by a slice (`&[u8]`).
    #[doc(hidden)]
    Slice {
        #[doc(hidden)]
        pub _bytes: &'a [u8],
    },
}

impl<'a> fmt::Show for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Owned { ref _bytes } => f.write(_bytes.as_slice()),
            Slice { ref _bytes } => f.write(*_bytes),
        }
    }
}

impl<'a> PartialOrd for Token<'a> {
    #[inline]
    fn partial_cmp(&self, other: &Token<'a>) -> Option<Ordering> {
        self.as_bytes().partial_cmp(&other.as_bytes())
    }
}

impl<'a> Ord for Token<'a> {
    #[inline]
    fn cmp(&self, other: &Token<'a>) -> Ordering {
        self.as_bytes().cmp(&other.as_bytes())
    }
}

impl<'a> PartialEq for Token<'a> {
    #[inline]
    fn eq(&self, other: &Token<'a>) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'a> Eq for Token<'a> { }

impl<'a> Collection for Token<'a> {
    #[inline]
    fn len(&self) -> uint {
        self.as_bytes().len()
    }
}

impl Token<'static> {
    /// Create a `Token` from a sequence of bytes.
    ///
    /// Returns `Err` with the original vector if not every byte in the vector
    /// is an RFC 7230 `tchar`.
    #[inline]
    pub fn from_vec(vec: Vec<u8>) -> Result<Token<'static>, Vec<u8>> {
        if vec.iter().all(|&c| is_tchar(c)) {
            Ok(Owned { _bytes: vec })
        } else {
            Err(vec)
        }
    }

    /// Create a `Token` from a sequence of bytes, without checking it.
    ///
    /// Be very careful calling this.
    #[inline]
    pub unsafe fn from_vec_nocheck(vec: Vec<u8>) -> Token<'static> {
        Owned { _bytes: vec }
    }
}

impl<'a> Token<'a> {
    /// Create a `Token` from a sequence of bytes.
    ///
    /// Returns `None` if not every byte in the slice is a RFC 7230 `tchar`.
    pub fn from_slice(slice: &'a [u8]) -> Option<Token<'a>> {
        if slice.iter().all(|&c| is_tchar(c)) {
            Some(Slice { _bytes: slice })
        } else {
            None
        }
    }

    /// Create a `Token` from a sequence of bytes, without checking it.
    ///
    /// Be very careful calling this.
    pub unsafe fn from_slice_nocheck(slice: &'a [u8]) -> Token<'a> {
        Slice { _bytes: slice }
    }

    /// Make a copy of the token, based around a slice of `self`.
    ///
    /// This is practically a free operation.
    #[inline]
    pub fn slice<'b>(&'b self) -> Token<'b> {
        Slice { _bytes: self.as_bytes() }
    }

    /// Change a slice token into an owned token.
    ///
    /// An owned token will be unchanged.
    #[inline]
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Owned { _bytes } => Owned { _bytes: _bytes },
            Slice { _bytes } => Owned { _bytes: _bytes.to_vec() },
        }
    }

    /// Get a string slice of the contents of the token.
    #[inline]
    pub fn as_str<'b>(&'b self) -> &'b str {
        // `token` is a subset of ASCII, so this cannot produce invalid data.
        unsafe {
            str::raw::from_utf8(self.as_bytes())
        }
    }

    /// Get a slice of the bytes in the token.
    #[inline]
    pub fn as_bytes<'b>(&'b self) -> &'b [u8] {
        match *self {
            Owned { ref _bytes } => _bytes.as_slice(),
            Slice { ref _bytes } => _bytes.as_slice(),
        }
    }
}
