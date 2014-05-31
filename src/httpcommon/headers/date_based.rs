//! defines all headers that contain dates
extern crate time;

use std;
use std::io::IoResult;
use std::str::{SendStr, Slice};
use std::to_str::ToStr;
use std::from_str::from_str;
use self::time::{strptime, Tm};
use super::{Header, HeaderMarker};

header!(#[doc="The Expires entity-header field gives the date/time after which the response is considered stale."]
        EXPIRES, "expires", Expires)

header!(#[doc="The Date general-header field represents the date and time at which the message was originated."]
        DATE, "date", Tm)

header!(#[doc="The If-Modified-Since request-header field is used with a method to make it conditional: if the requested variant has not been modified since the time specified in this field, an entity will not be returned from the server; instead, a 304 (not modified) response will be returned without any message-body."]
        IF_MODIFIED_SINCE, "if-modified-since", Tm)

header!(#[doc="The If-Unmodified-Since request-header field is used with a method to make it conditional. If the requested resource has not been modified since the time specified in this field, the server SHOULD perform the requested operation as if the If-Unmodified-Since header were not present."]
        IF_UNMODIFIED_SINCE, "if-unmodified-since", Tm)

header!(#[doc="The Last-Modified entity-header field indicates the date and time at which the origin server believes the variant was last modified."]
        LAST_MODIFIED, "last-modified", Tm)

header!(#[doc="The Retry-After response-header field can be used with a 503 (Service Unavailable) response to indicate how long the service is expected to be unavailable to the requesting client."]
        RETRY_AFTER, "retry-after", RetryAfter)

impl Header for uint {
    fn parse_header(raw: &[Vec<u8>]) -> Option<uint> {
        let raw = require_single_field!(raw);
        let raw = match std::str::from_utf8(raw) {
            Some(raw) => raw,
            None => return None,
        };
        from_str::<uint>(raw)
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
    DeltaRA(uint),
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

    #[test]
    fn test_retry() {
        let now = time::now();
        {
            let now_raw = fmt_header(&now);
            let h: Option<RetryAfter> = Header::parse_header([now_raw]);
            assert_eq!(Some(DateRA(now.clone())), h);
        }

        {
            let now1_raw = fmt_header(&now);
            let now2_raw = fmt_header(&now);
            let h: Option<RetryAfter> = Header::parse_header([now1_raw, now2_raw]);
            assert_eq!(None, h);
        }

        {
            let h: Option<RetryAfter> = Header::parse_header([Vec::from_slice(bytes!("foo"))]);
            assert_eq!(None, h);
        }

        {
            let h: Option<RetryAfter> = Header::parse_header([Vec::from_slice(bytes!("42"))]);
            assert_eq!(Some(DeltaRA(42u)), h);
        }

        {
            let h: Option<RetryAfter> = Header::parse_header([Vec::from_slice(bytes!("42")),
                                                              Vec::from_slice(bytes!("24"))]);
            assert_eq!(None, h);
        }

        {
            let h: Option<RetryAfter> = Header::parse_header([Vec::from_slice(bytes!("-42"))]);
            assert_eq!(None, h);
        }
    }
}
