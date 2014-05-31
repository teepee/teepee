//! defines all headers that contain dates
extern crate time;

use std;
use std::io::IoResult;
use std::str::{SendStr, Slice};
use std::to_str::ToStr;
use std::from_str::from_str;
use self::time::{strptime, Tm};
use super::{Header, HeaderMarker};

// I dunno
macro_rules! require_single_field {
    ($field_values:expr) => ({
        let mut iter = $field_values.iter();
        match (iter.next(), iter.next()) {
            (Some(ref field_value), None) => field_value.as_slice(),
            _ => return None,
        }
    })
}

/// defines a struct and a ``HeaderMarker`` for it so that together they
/// identify the same header type
macro_rules! header {
    ($struct_ident:ident, $header_name:expr, $output_type:ty) => (
        #[allow(missing_doc)]
        pub struct $struct_ident;
 
        impl HeaderMarker<$output_type> for $struct_ident {
            fn header_name(&self) -> SendStr {
                Slice($header_name)
            }
        }
    )
}

header!(EXPIRES, "expires", Expires)
header!(DATE, "date", Tm)
header!(IF_MODIFIED_SINCE, "if-modified-since", Tm)
header!(IF_UNMODIFIED_SINCE, "if-unmodified-since", Tm)
header!(LAST_MODIFIED, "last-modified", Tm)
header!(RETRY_AFTER, "retry-after", RetryAfter)

impl Header for int {
    fn parse_header(raw: &[Vec<u8>]) -> Option<int> {
        let raw = require_single_field!(raw);
        let raw = match std::str::from_utf8(raw) {
            Some(raw) => raw,
            None => return None,
        };
        from_str::<int>(raw)
    }
    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        write!(w, "{}", self.to_str())
    }
}

/// The data type for the ``expires`` header.
#[deriving(Clone, Eq, Show)]
pub enum Expires {
    /// The Expires header had an invalid format, which MUST be interpreted as “in the past”.
    Past,
    /// A valid Expires header date.
    ExpiresDate(Tm),
}

impl Header for Expires {
    fn parse_header(raw: &[Vec<u8>]) -> Option<Expires> {
        let _ = require_single_field!(raw);
        match Header::parse_header(raw) {
            Some(tm) => Some(ExpiresDate(tm)),
            None => Some(Past),
        }
    }
 
    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        match *self {
            Past => write!(w, "0"),
            ExpiresDate(ref tm) => tm.fmt_header(w),
        }
    }
}

impl Header for Tm {
    fn parse_header(raw: &[Vec<u8>]) -> Option<Tm> {
        let raw = require_single_field!(raw);
        let raw = match std::str::from_utf8(raw) {
            Some(raw) => raw,
            None => return None,
        };
        // XXX: %Z actually ignores any timezone other than UTC. Probably not a good idea?
        match strptime(raw, "%a, %d %b %Y %T %Z") {  // RFC 822, updated by RFC 1123
            Ok(time) => return Some(time),
            Err(_) => ()
        }

        match strptime(raw, "%a, %d %b %Y %T %z") {  // RFC 822, updated by RFC 1123
            Ok(time) => return Some(time),
            Err(_) => ()
        }
 
        match strptime(raw, "%A, %d-%b-%y %T %Z") {  // RFC 850, obsoleted by RFC 1036
            Ok(time) => return Some(time),
            Err(_) => ()
        }
 
        match strptime(raw, "%c") {  // ANSI C's asctime() format
            Ok(time) => Some(time),
            Err(_) => None,
        }
    }
 
    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        write!(w, "{}", self.to_utc().strftime("%a, %d %b %Y %T GMT"))
    }
}

/// The data type for the ``Retry-After`` header.
#[deriving(Clone, Eq, Show)]
pub enum RetryAfter {
    /// A valid Retry-After header date.
    DateRA(Tm),
    /// A valid Retry-After header delta value.
    DeltaRA(int),
}

impl Header for RetryAfter {
    fn parse_header(raw: &[Vec<u8>]) -> Option<RetryAfter> {
        let _ = require_single_field!(raw);
        match Header::parse_header(raw) {
            Some(tm) => Some(DateRA(tm)),
            None => match Header::parse_header(raw) {
                Some(delta) => Some(DeltaRA(delta)),
                None => None,
            }
        }
    }
 
    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        match *self {
            DeltaRA(ref delta) => delta.fmt_header(w),
            DateRA(ref tm) => tm.fmt_header(w),
        }
    }
}

#[cfg(test)]
mod tests {
    use std;
    use super::time;
    use super::*;
    use super::super::{Header, Headers, fmt_header};

    fn expect<H: Header + std::fmt::Show + Eq>(h: Option<H>, h_expected: H, raw: &[u8]) {
        let h = h.unwrap();
        assert_eq!(fmt_header(&h).as_slice(), raw);
        assert_eq!(h, h_expected);
    }

    fn expect_none<H: Header>(h: Option<H>) {
        assert!(h.is_none());
    }
 
    #[test]
    fn test_expires() {
        let mut headers = Headers::new();
        expect_none(headers.get(EXPIRES));
        headers.set(EXPIRES, Past);
        expect(headers.get(EXPIRES), Past, bytes!("0"));
        //assert_eq!(headers.get_raw("expires"), vec![vec!['0' as u8]]);
        expect(headers.get(EXPIRES), Past, bytes!("0"));
        headers.remove(&EXPIRES);
        expect_none(headers.get(EXPIRES));
     
        expect_none(headers.get(DATE));
        let now = time::now();
        let now_raw = fmt_header(&now);
        headers.set(DATE, now.clone());
        expect(headers.get(DATE), now.clone(), now_raw.as_slice());
    }
}
