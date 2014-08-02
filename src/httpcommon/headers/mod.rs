//! HTTP headers.

use std::any::{AnyRefExt};
use std::mem::{transmute, transmute_copy};
use std::intrinsics::TypeId;
use std::fmt;
use std::io::{MemWriter, IoResult};
use std::raw::TraitObject;
use std::str::SendStr;

use std::collections::hashmap::HashMap;

use self::internals::Item;

pub use self::date_based::{EXPIRES, Expires, DATE, IF_MODIFIED_SINCE,
                           IF_UNMODIFIED_SINCE, LAST_MODIFIED,
                           RETRY_AFTER, RetryAfter, AGE};

/// helper that simultaneously fetches the first raw header value, and
/// ensures that we only successfully parse if there is one and only
/// one such value.
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
    (#[$attr:meta] $struct_ident:ident, $header_name:expr, $output_type:ty) => (
        #[allow(missing_doc)]
        #[allow(non_camel_case_types)]
        #[$attr] 
        pub struct $struct_ident;
 
        impl HeaderMarker<$output_type> for $struct_ident {
            fn header_name(&self) -> SendStr {
                Slice($header_name)
            }
        }
    )
}

pub mod date_based;
mod internals;

/// An inner trait to ensure that only this module can call `get_type_id()`.
trait AnyPrivate {
    /// Get the `TypeId` of `self`
    fn get_type_id(&self) -> TypeId;
}

impl<T: 'static> AnyPrivate for T {
    fn get_type_id(&self) -> TypeId { TypeId::of::<T>() }
}

/// The data type of an HTTP header for encoding and decoding.
pub trait Header: AnyPrivate {
    /// Parse a header from one or more header field values, returning some value if successful or
    /// `None` if parsing fails.
    ///
    /// Most headers only accept a single header field (i.e. they should return `None` if the outer
    /// slice contains other than one value), but some may accept multiple header field values; in
    /// such cases, they MUST be equivalent to having them all as a comma-separated single field
    /// (RFC 7230, section 3.3.2 Field Order), with exceptions for things like dropping invalid values.
    fn parse_header(raw_field_values: &[Vec<u8>]) -> Option<Self>;

    /// Introducing an `Err` value that does *not* come from the writer is incorrect behaviour and
    /// may lead to task failure in certain situations. (The primary case where this will happen is
    /// accessing a cached Box<Header> object as a different type; then, it is shoved into a buffer
    /// through fmt_header and then back into the new type through parse_header. Should the
    /// fmt_header call have return an Err, it will fail.
    fn fmt_header(&self, writer: &mut Writer) -> IoResult<()>;
}

// impl copied from std::any. Not especially nice, sorry :-(
impl<'a> AnyRefExt<'a> for &'a Header {
    #[inline]
    fn is<T: 'static>(self) -> bool {
        // Get TypeId of the type this function is instantiated with
        let t = TypeId::of::<T>();

        // Get TypeId of the type in the trait object
        let boxed = self.get_type_id();

        // Compare both TypeIds on equality
        t == boxed
    }

    #[inline]
    fn downcast_ref<T: 'static>(self) -> Option<&'a T> {
        if self.is::<T>() {
            Some(unsafe { self.downcast_ref_unchecked() })
        } else {
            None
        }
    }
}

/// An extension of `AnyRefExt` allowing unchecked downcasting of trait objects to `&T`.
trait UncheckedAnyRefExt<'a> {
    /// Returns a reference to the boxed value, assuming that it is of type `T`. This should only be
    /// called if you are ABSOLUTELY CERTAIN of `T` as you will get really wacky output if it’s not.
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T;
}

impl<'a> UncheckedAnyRefExt<'a> for &'a Header {
    #[inline]
    unsafe fn downcast_ref_unchecked<T: 'static>(self) -> &'a T {
        // Get the raw representation of the trait object
        let to: TraitObject = transmute_copy(&self);

        // Extract the data pointer
        transmute(to.data)
    }
}

/// An extension of `AnyMutRefExt` allowing unchecked downcasting of trait objects to `&mut T`.
trait UncheckedAnyMutRefExt<'a> {
    /// Returns a reference to the boxed value, assuming that it is of type `T`. This should only be
    /// called if you are ABSOLUTELY CERTAIN of `T` as you will get really wacky output if it’s not.
    unsafe fn downcast_mut_unchecked<T: 'static>(self) -> &'a mut T;
}

impl<'a> UncheckedAnyMutRefExt<'a> for &'a mut Header {
    #[inline]
    unsafe fn downcast_mut_unchecked<T: 'static>(self) -> &'a mut T {
        // Get the raw representation of the trait object
        let to: TraitObject = transmute_copy(&self);

        // Extract the data pointer
        transmute(to.data)
    }
}

/// A header marker, providing the glue between the header name and a type for that header.
///
/// Standard usage of this is very simple unit-struct marker types, like this:
///
/// ```rust,ignore
/// // The header data type
/// #[deriving(Clone)]
/// pub struct Foo {
///     ...
/// }
///
/// impl Header for Foo {
///     ...
/// }
///
/// // The marker type for accessing the header and specifying the name
/// pub struct FOO;
///
/// impl HeaderMarker<Foo> for FOO {
///     fn header_name(&self) -> SendStr {
///         Slice("foo")
///     }
/// }
/// ```
///
/// Then, accessing the header is done like this:
///
/// ```rust
/// # use std;
/// # #[deriving(Clone)] struct Foo;
/// # impl httpcommon::headers::Header for Foo {
/// #     fn parse_header(_raw: &[Vec<u8>]) -> Option<Foo> { Some(Foo) }
/// #     fn fmt_header(&self, w: &mut Writer) -> std::io::IoResult<()> { Ok(()) }
/// # }
/// # struct FOO;
/// # impl httpcommon::headers::HeaderMarker<Foo> for FOO {
/// #     fn header_name(&self) -> std::str::SendStr { std::str::Slice("foo") }
/// # }
/// # struct Request { headers: httpcommon::headers::Headers }
/// # let mut request = Request { headers: httpcommon::headers::Headers::new() };
/// # request.headers.set(FOO, Foo);
/// // Of course, this is assuming that we *know* the header is there
/// let foo = request.headers.get(FOO).unwrap();
/// request.headers.set(FOO, foo);
/// ```
///
/// And lo! `foo` is a `Foo` object corresponding to the `foo` (or `Foo`, or `fOO`, &c.) header in
/// the request.
///
/// Authors are strongly advised that they should not implement `HeaderMarker<T>` for more than one
/// `T` on the same type; as well as going against the spirit of things, it would also require
/// explicit type specifiation every time, which would be a nuisance.
pub trait HeaderMarker<OutputType: Header + 'static> {
    /// The name of the header that shall be used for retreiving and setting.
    ///
    /// Normally this will be a static string, but occasionally it may be necessary to define it at
    /// runtime, for dynamic header handling.
    fn header_name(&self) -> SendStr;
}

impl Header for Box<Header + 'static> {
    fn parse_header(_raw: &[Vec<u8>]) -> Option<Box<Header + 'static>> {
        // Dummy impl; XXX: split to ToHeader/FromHeader?
        None
    }

    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        self.fmt_header(w)
    }
}

impl<'a> Header for &'a Header {
    fn parse_header(_raw: &[Vec<u8>]) -> Option<&'a Header> {
        // Dummy impl; XXX: split to ToHeader/FromHeader?
        None
    }

    fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
        self.fmt_header(w)
    }
}

/// A collection of HTTP headers.
///
/// Usage
/// -----
///
/// The primary methods you will care about are:
///
/// Typed header access
/// ```````````````````
///
/// Unlike most HTTP libraries, this one cares about correctness and strong typing; headers are
/// semantically typed values, not just sequences of characters. This may lead to some surprises for
/// people used to other environments. For example, you might think that the `Connection` header is
/// a scalar, having seen it with the value `close` most frequently when present; therefore you
/// might expect to check `*request.headers.get_ref(CONNECTION) == Close`. Well, it's not: it's
/// actually a linear value, `Vec<Connection>` instead of `Connection`, so you'll actually be
/// wanting to check something more like `request.headers.get_ref(CONNECTION).map(|c|
/// c.contains(&Close)) == Some(true)`. Yes, this is more cumbersome than what you might write in
/// another language, such as `request.headers["Connection"] == "close"`, but it's actually correct,
/// whereas the one people would often write is very definitely incorrect.
///
/// There are four methods for this:
///
/// - `get`: cloned value, if it exists.
/// - `get_ref`: reference to the value, if it exists.
/// - `get_mut_ref`: mutable reference to the value, if it exists.
/// - `set`: assign the value.
///
/// One thing out of the ordinary to be aware of is that all of these methods take `&mut self`, even
/// `get` and `get_ref`; this is not ideal, but it is thus for a very good reason, an outcome of the
/// hybrid typed/raw approach employed. The main practical effect of this is that you cannot take
/// references to more than one header at once; where possible, use `get_ref`, but it is
/// acknowledged that it will not always be feasible to use it: this is why `get` exists, which
/// clones the value, thus releasing the lock on the header collection.
///
/// Raw header access
/// `````````````````
///
/// Largely you should prefer typed access, but sometimes raw header access is convenient or even
/// necessary.
///
/// There are again four methods for this, corresponding to the typed techniques:
///
/// - `get_raw`: cloned value, if it exists.
/// - `get_raw_ref`: reference to the value, if it exists.
/// - `get_raw_mut_ref`: mutable reference to the value, if it exists.
/// - `set_raw`: assign the value.
///
/// Aside: what is a header?
/// ------------------------
///
/// When we speak of a header in this library, we are not referring to the HTTP concept of a *header
/// field*; we are dealing with a slightly higher abstraction than that.
///
/// In HTTP/1.1, a message header is defined like this (RFC 7230, section 3.2 Header Fields):
///
/// ```ignore
///     header-field   = field-name ":" OWS field-value OWS
///
///     field-name     = token
///     field-value    = *( field-content / obs-fold )
///     field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
///     field-vchar    = VCHAR / obs-text
///
///     obs-fold       = CRLF 1*( SP / HTAB )
///                    ; obsolete line folding
///                    ; see Section 3.2.4    message-header = field-name ":" [ field-value ]
/// ```
///
/// This is something all web developers should be at least basically familiar with.
///
/// The interesting part comes a little later in that section and is to do with how message headers
/// *combine*:
///
/// ```ignore
/// A sender MUST NOT generate multiple header fields with the same field
/// name in a message unless either the entire field value for that
/// header field is defined as a comma-separated list [i.e., #(values)]
/// or the header field is a well-known exception (as noted below).
///
/// A recipient MAY combine multiple header fields with the same field
/// name into one "field-name: field-value" pair, without changing the
/// semantics of the message, by appending each subsequent field value to
/// the combined field value in order, separated by a comma.  The order
/// in which header fields with the same field name are received is
/// therefore significant to the interpretation of the combined field
/// value; a proxy MUST NOT change the order of these field values when
/// forwarding a message.
/// ```
///
/// In this library, what we call a header is not a single message header, but rather the
/// combination of all message headers with the same field name; that is, a field-name plus *all*
/// related field-values. One can still access them separately through the raw interface, but the
/// preferred technique is accessing them through the typed interface, where all such
/// identically-named message headers will be merged.
///
/// Representation
/// --------------
///
/// A knowledge of how headers are represented internally may assist you in using them efficiently.
///
/// Headers are stored in a hash map; the keys are header names and the values are what for the
/// purpose of this description will be dubbed *items*.
///
/// At this point it is worth recalling that in a request or response, there can be multiple header
/// fields with the same name; this is why the raw representation of each header item is `Vec<Vec<u8>>`
/// rather than `Vec<u8>` each header field can 
/// Each header name is thus associated with an
/// item.
pub struct Headers {
    data: HashMap<SendStr, Item>,
}

impl Headers {
    /// Construct a new header collection.
    pub fn new() -> Headers {
        Headers {
            data: HashMap::new(),
        }
    }
}

impl<H: Header + Clone + 'static, M: HeaderMarker<H>> Headers {

    /// Retrieve a header value. The value is a clone of the one that is stored internally.
    ///
    /// The interface is strongly typed; see TODO for a more detailed explanation of how it works.
    pub fn get(&mut self, header_marker: M) -> Option<H> {
        self.get_ref(header_marker).map(|h: &H| h.clone())
    }

    /// Get a reference to a header value.
    ///
    /// Bear in mind that because of the internals, this method (and also `get` and `get_mut_ref`)
    /// takes `&mut self`; in consequence, you won't be able to take references to two headers at
    /// once. That is, in fact, why `get` is there---to provide a convenient way to avoid that
    /// problem, by immediately cloning the header value.
    ///
    /// The interface is strongly typed; see TODO for a more detailed explanation of how it works.
    pub fn get_ref<'a>(&'a mut self, header_marker: M) -> Option<&'a H> {
        self.data.find_mut(&header_marker.header_name()).and_then(|item| item.typed_ref())
    }

    /// Get a mutable reference to a header value.
    ///
    /// The interface is strongly typed; see TODO for a more detailed explanation of how it works.
    pub fn get_mut_ref<'a>(&'a mut self, header_marker: M) -> Option<&'a mut H> {
        self.data.find_mut(&header_marker.header_name()).and_then(|item| item.typed_mut_ref())
    }

    /// Set the named header to the given value.
    pub fn set(&mut self, header_marker: M, value: H) {
        let _ = self.data.find_with_or_insert_with(header_marker.header_name(), value,
                                                   |_k, item, value| item.set_typed(value),
                                                   |_k, value| Item::from_typed(value));
    }

    /// Get the raw values of a header, by name.
    ///
    /// The returned value is a slice of each header field value.
    #[inline]
    pub fn get_raw(&mut self, header_marker: M) -> Option<Vec<Vec<u8>>> {
        self.get_raw_ref(header_marker).map(|h| h.iter().map(|v| v.clone()).collect())
    }

    /// Get the raw values of a header, by name.
    ///
    /// The returned value is a slice of each header field value.
    #[inline]
    pub fn get_raw_ref<'a>(&'a mut self, header_marker: M) -> Option<&'a [Vec<u8>]> {
        self.data.find_mut(&header_marker.header_name()).map(|item| item.raw_ref().as_slice())
    }

    /// Get a mutable reference to the raw values of a header, by name.
    ///
    /// The returned vector contains each header field value.
    #[inline]
    pub fn get_raw_mut_ref<'a>(&'a mut self, header_marker: M) -> Option<&'a mut Vec<Vec<u8>>> {
        self.data.find_mut(&header_marker.header_name()).map(|item| item.raw_mut_ref())
    }

    /// Set the raw value of a header, by name.
    ///
    /// This invalidates the typed representation.
    #[inline]
    pub fn set_raw(&mut self, header_marker: M, value: Vec<Vec<u8>>) {
        let _ = self.data.find_with_or_insert_with(header_marker.header_name(), value,
                                                   |_k, item, value| item.set_raw(value),
                                                   |_k, value| Item::from_raw(value));
    }

    /// Remove a header from the collection.
    /// Returns true if the named header was present.
    pub fn remove(&mut self, header_marker: &M) {
        self.data.remove(&header_marker.header_name());
    }
}

/// An adapter which provides `std::fmt::Show` as equivalent to `Header.fmt_header`, so that you can
/// actually *use* the thing.
struct HeaderShowAdapter<'a, H>(pub &'a H);

impl<'a, H: Header> fmt::Show for HeaderShowAdapter<'a, H> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let HeaderShowAdapter(h) = *self;
        match h.fmt_header(f) {
            Ok(v) => Ok(v),
            Err(_) => Err(fmt::WriteError)
        }
    }
}

#[inline]
/// Convert a typed header into the raw HTTP header field value.
pub fn fmt_header<H: Header>(h: &H) -> Vec<u8> {
    let mut output = MemWriter::new();
    // Result.unwrap() is correct here, for MemWriter won’t make an IoError,
    // and fmt_header is not permitted to introduce one of its own.
    h.fmt_header(&mut output).unwrap();
    output.unwrap()
}

#[cfg(test_broken)]
mod tests {
    use super::*;

    fn expect<H: Header + std::fmt::Show + Eq>(h: Option<H>, h_expected: H, raw: &[u8]) {
        let h = h.unwrap();
        assert_eq!(fmt_header(&h).as_slice(), raw);
        assert_eq!(h, h_expected);
    }

    #[test]
    fn test_basics() {
        let mut headers = Headers::new();

        assert_eq!(headers.get(EXPIRES), None);

        headers.set(EXPIRES, Past);
        assert_eq!(headers.mostly_get(&EXPIRES), &mut Typed(box Past));
        expect(headers.get(EXPIRES), Past, b"0");
        assert_eq!(headers.get_raw("expires"), vec![vec![b'0']]);
        expect(headers.get(EXPIRES), Past, b"0");

        headers.remove(&EXPIRES);
        assert_eq!(headers.get(EXPIRES), None);

        assert_eq!(headers.get(DATE), None);
        let now = time::now();
        let now_raw = fmt_header(&now);
        headers.set(DATE, now.clone());
        expect(headers.get(DATE), now.clone(), now_raw.as_slice());
    }
}
