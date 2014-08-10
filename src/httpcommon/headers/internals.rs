//! The internals of header representation. That is: `Item`.

use std::any::AnyRefExt;

use super::{Header, UncheckedAnyMutRefExt, fmt_header};

/// All the header field values, raw or typed, with a shared field name.
///
/// Each item contains a raw and/or a typed representation (but not neither!); for safety, taking a
/// mutable reference to either invalidates the other; immutable references to not cause
/// invalidation and so should be preferred where possible.
///
/// If a typed representation is invalidated, it is immediately dropped and replaced with `None`; if
/// a raw representation is invalidated, it is *not* dropped, but rather marked as invalid: this is
/// so that the outer vector can be reused, reducing allocation churn slightly. Trivial performance
/// improvement, though it also increases memory usage in the mean time. That is expected to be a
/// very small amount (<1KB across all headers) in most cases and acceptable.
///
/// Invariants beyond those enforced by the type system:
///
/// - `raw == None` requires `!raw_valid`.
/// - `raw == None && typed == None` is not legal.
/// - `raw_valid == true` requires `raw` to be some with `.len() > 0`.
pub struct Item {
    /// Whether the raw header form is valid. If a mutable reference is taken to `typed`, this will
    /// be set to `false`, meaning that for the purposes of reading, `raw` must be considered to be
    /// invalid and must be produced once again. This exists as a slight efficiency improvement over
    /// just resetting `raw` to `None` in that, if the raw form is read again, the raw vectors can
    /// be reused; this is almost certain to be faster than dropping the vectors and creating new
    /// ones.
    raw_valid: bool,

    /// A raw, unparsed header. Each item in the outer vector is a header field value, the names of
    /// which were equivalent. Each inner vector is a string in the ISO-8859-1 character set, but
    /// could contain things in other character sets according to the rules of RFC 2047, e.g. in a
    /// *TEXT rule (RFC 2616 grammar).
    ///
    /// Reading from this is only valid if `raw_valid` is `true`.
    raw: Option<Vec<Vec<u8>>>,

    /// A strongly typed header which has been parsed from the raw value.
    typed: Option<Box<Header + 'static>>,
}

impl Item {
    /// Construct a new Item from a raw representation.
    ///
    /// The vector given MUST contain at least one value, or this will fail.
    pub fn from_raw(raw: Vec<Vec<u8>>) -> Item {
        assert!(raw.len() > 0);
        Item {
            raw_valid: true,
            raw: Some(raw),
            typed: None,
        }
    }

    /// Construct a new Item from a typed representation.
    pub fn from_typed<H: Header + 'static>(typed: H) -> Item {
        Item {
            raw_valid: false,
            raw: None,
            typed: Some(box typed as Box<Header + 'static>),
        }
    }

    fn raw_mut_ref_internal<'a>(&'a mut self, invalidate_typed: bool) -> &'a mut Vec<Vec<u8>> {
        if self.raw_valid == true {
            // All is good; we'll return the value in good time.
        } else {
            self.raw_valid = true;
            match (&mut self.raw, &mut self.typed) {
                (&Some(ref mut raw), &Some(ref typed)) => {
                    raw.truncate(1);
                    raw.as_mut_slice()[0] = fmt_header(typed);
                },

                (ref mut raw @ &None, &Some(ref typed)) => {
                    **raw = Some(vec![fmt_header(typed)]);
                },

                _ => unreachable!(),
            }
        }
        if invalidate_typed {
            self.typed = None;
        }
        self.raw.get_mut_ref()
    }

    /// Get a mutable reference to the raw representation of the header values.
    ///
    /// Because you may modify the raw representation through this mutable reference, calling this
    /// invalidates the typed representation; next time you want to access the value in typed
    /// fashion, it will be parsed from the raw form.
    ///
    /// Only use this if you need to mutate the raw form; if you don't, use `raw_ref`.
    pub fn raw_mut_ref<'a>(&'a mut self) -> &'a mut Vec<Vec<u8>> {
        self.raw_mut_ref_internal(true)
    }

    /// Get a reference to the raw representation of the header values.
    ///
    /// If a valid raw representation exists, it will be used, making this a very cheap operation;
    /// if it does not, then the typed representation will be converted to raw form and you will
    /// then get a reference to that. In summary, it doesn't much matter; you'll get your raw
    /// reference.
    ///
    /// See also `raw_mut_ref`, if you wish to mutate the raw representation.
    pub fn raw_ref<'a>(&'a mut self) -> &'a Vec<Vec<u8>> {
        &*self.raw_mut_ref_internal(false)
    }

    /// Set the raw form of the header.
    ///
    /// This invalidates the typed representation.
    pub fn set_raw(&mut self, raw: Vec<Vec<u8>>) {
        self.raw_valid = true;
        self.raw = Some(raw);
        self.typed = None;
    }

    fn typed_mut_ref_internal<'a, H: Header + 'static>
                             (&'a mut self, invalidate_raw: bool) -> Option<&'a mut H> {
        match self.typed {
            None => {
                debug_assert_eq!(self.raw_valid, true);
                debug_assert!(self.raw.is_some());
                if invalidate_raw {
                    self.raw_valid = false;
                }
                let h: Option<H> = Header::parse_header(self.raw.get_ref().as_slice());
                match h {
                    Some(h) => {
                        self.typed = Some(box h as Box<Header + 'static>);
                        Some(unsafe { self.typed.get_mut_ref().downcast_mut_unchecked::<H>() })
                    },
                    None => None,
                }
            },
            Some(ref mut h) if h.is::<H>() => {
                if invalidate_raw {
                    self.raw_valid = false;
                }
                Some(unsafe { h.downcast_mut_unchecked::<H>() })
            },
            Some(ref mut h) => {
                if !self.raw_valid {
                    match self.raw {
                        Some(ref mut raw) => {
                            raw.truncate(1);
                            raw.as_mut_slice()[0] = fmt_header(h);
                        },
                        None => {
                            self.raw = Some(vec![fmt_header(h)]);
                        },
                    }
                    self.raw_valid = !invalidate_raw;
                } else if invalidate_raw {
                    self.raw_valid = false;
                }
                let otyped: Option<H> = Header::parse_header(self.raw.get_ref().as_slice());
                match otyped {
                    Some(typed) => {
                        *h = box typed as Box<Header + 'static>;
                        Some(unsafe { h.downcast_mut_unchecked::<H>() })
                    },
                    None => None,
                }
            },
        }
    }

    /// Get a mutable reference to the typed representation of the header values.
    ///
    /// Because you may modify the typed representation through this mutable reference, calling
    /// this invalidates the raw representation; next time you want to access the value in raw
    /// fashion, it will be produced from the typed form.
    ///
    /// Only use this if you need to mutate the typed form; if you don't, use `typed_ref`.
    pub fn typed_mut_ref<'a, H: Header + 'static>(&'a mut self) -> Option<&'a mut H> {
        self.typed_mut_ref_internal(true)
    }

    /// Get a reference to the typed representation of the header values.
    ///
    /// If a valid typed representation exists, it will be used, making this a very cheap operation;
    /// if it does not, then the raw representation will be converted to typed form and you will
    /// then get a reference to that. In summary, it doesn't much matter; you'll get your typed
    /// reference.
    ///
    /// See also `typed_mut_ref`, if you wish to mutate the typed representation.
    pub fn typed_ref<'a, H: Header + 'static>(&'a mut self) -> Option<&'a H> {
        self.typed_mut_ref_internal(false).map(|h| &*h)
    }

    /// Set the typed form of the header.
    ///
    /// This invalidates the raw representation.
    pub fn set_typed<'a, H: Header + 'static>(&mut self, value: H) {
        self.raw_valid = false;
        self.typed = Some(box value as Box<Header + 'static>);
    }
}

#[cfg(test)]
mod tests {
    use super::Item;
    use super::super::Header;
    use std::fmt;
    use std::any::AnyRefExt;
    use std::io::IoResult;

    // Until https://github.com/mozilla/rust/issues/9052 is fixed, the super:: is needed.
    #[allow(unnecessary_qualification)]
    impl super::Item {
        fn assert_invariants(&self) {
            assert!(self.raw.is_some() || !self.raw_valid);
            assert!(self.raw.is_some() || self.typed.is_some());
            assert!(!self.raw_valid || self.raw.get_ref().len() > 0);
        }
    }

    fn mkitem<H: Header + 'static>(raw_valid: bool,
                                   raw: Option<Vec<Vec<u8>>>,
                                   typed: Option<H>) -> Item {
        let item = Item {
            raw_valid: raw_valid,
            raw: raw,
            typed: typed.map(|h| box h as Box<Header + 'static>),
        };
        item.assert_invariants();
        item
    }

    #[deriving(PartialEq, Eq, Clone, Show)]
    struct StrongType(Vec<Vec<u8>>);
    #[allow(non_camel_case_types)]
    type st = StrongType;

    impl Header for StrongType {
        fn parse_header(raw: &[Vec<u8>]) -> Option<StrongType> {
            Some(StrongType(raw.iter().map(|x| x.clone()).collect()))
        }

        fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
            let StrongType(ref vec) = *self;
            let mut first = true;
            for field in vec.iter() {
                if !first {
                    try!(w.write(b", "))
                }
                try!(w.write(field.as_slice()))
                first = false;
            }
            Ok(())
        }
    }

    #[deriving(PartialEq, Eq, Clone, Show)]
    struct NonParsingStrongType(StrongType);
    #[allow(non_camel_case_types)]
    type np = NonParsingStrongType;

    impl Header for NonParsingStrongType {
        fn parse_header(_raw: &[Vec<u8>]) -> Option<NonParsingStrongType> {
            None
        }

        fn fmt_header(&self, w: &mut Writer) -> IoResult<()> {
            let NonParsingStrongType(ref st) = *self;
            st.fmt_header(w)
        }
    }

    fn assert_headers_eq<H: Header + Clone + PartialEq + fmt::Show + 'static>(item: &Item, other: &Item) {
        item.assert_invariants();
        assert_eq!(item.raw_valid, other.raw_valid);
        assert_eq!(item.raw, other.raw);
        if item.typed.is_some() || other.typed.is_some() {
            let it = item.typed.get_ref();
            let ot = other.typed.get_ref();
            let ir = it.downcast_ref::<H>().expect("assert_headers_eq: expected Some item, got None");
            let or = ot.downcast_ref::<H>().expect("assert_headers_eq: expected Some other, got None");
            assert_eq!(ir, or);
        }
    }

    // Dummy 1: multiple headers
    fn d1raw() -> Vec<Vec<u8>> {
        vec![Vec::from_slice(b"ab"), Vec::from_slice(b"cd")]
        //vec![vec![b'a', b'b'], vec![b'c', b'd']]
    }
    fn d1st() -> StrongType { StrongType(d1raw()) }
    fn d1np() -> NonParsingStrongType { NonParsingStrongType(d1st()) }

    // Dummy 2: 1, but merged
    fn d2raw() -> Vec<Vec<u8>> {
        vec![Vec::from_slice(b"ab, cd")]
        //vec![vec![b'a', b'b', b',', b' ', b'c', b'd']]
    }
    //fn d2st() -> StrongType { StrongType(d2raw()) }
    //fn d2np() -> NonParsingStrongType { NonParsingStrongType(d2st()) }

    // Dummy 3: multiple headers, different from 1
    fn d3raw() -> Vec<Vec<u8>> {
        vec![Vec::from_slice(b"12"), Vec::from_slice(b"34")]
        //vec![vec![b'1', b'2'], vec![b'3', b'4']]
    }
    fn d3st() -> StrongType { StrongType(d3raw()) }
    fn d3np() -> NonParsingStrongType { NonParsingStrongType(d3st()) }

    // Dummy 4: 3, but merged
    fn d4raw() -> Vec<Vec<u8>> {
        vec![Vec::from_slice(b"12, 34")]
        //vec![vec![b'1', b'2', b',', b' ', b'3', b'4']]
    }
    fn d4st() -> StrongType { StrongType(d4raw()) }
    //fn d4np() -> NonParsingStrongType { NonParsingStrongType(d4st()) }

    #[test]
    #[should_fail]
    fn test_from_raw_with_empty_vector() {
        // Would not satisfy invariants, explicitly fails.
        let _raw = Item::from_raw(vec![]);
    }

    #[test]
    fn test_fresh_item_from_raw() {
        let item = Item::from_raw(vec![vec![]]);
        assert_headers_eq::<StrongType>(&item,
                                        &mkitem(true, Some(vec![vec![]]), None::<StrongType>));
    }

    #[test]
    fn test_fresh_item_from_typed() {
        let item = Item::from_typed(d1st());
        assert_headers_eq::<StrongType>(&item, &mkitem(false, None, Some(d1st())));
    }

    macro_rules! _thing {
        (1 $x:ident) => (Some(concat_idents!(d1, $x)()));
        (2 $x:ident) => (Some(concat_idents!(d2, $x)()));
        (3 $x:ident) => (Some(concat_idents!(d3, $x)()));
        (4 $x:ident) => (Some(concat_idents!(d4, $x)()));
        (- st) => (None::<StrongType>);
        (- np) => (None::<NonParsingStrongType>);
        (- $x:tt) => (None);
    }
    macro_rules! _bool((t)=>(true);(f)=>(false))
    macro_rules! _item {
        ($valid:tt, $raw:tt, $ty_n:tt $ty_ty:tt) => {
            mkitem(_bool!($valid), _thing!($raw raw), _thing!($ty_n $ty_ty))
        }
    }
    macro_rules! t {
        (
            $fn_name:ident =>
            ($s1:tt, $s2:tt, $s3a:tt $s3b:ident)
            $method:ident ( $(,$args:expr)* )
            ($e1:tt, $e2:tt, $e3a:tt $e3b:ident)
        ) => {
            #[test]
            fn $fn_name() {
                let mut item = _item!($s1, $s2, $s3a $s3b);
                let _ = item.$method($($args),*);
                assert_headers_eq::<StrongType>(&item, &_item!($e1, $e2, $e3a $e3b));
            }
        };
        (
            $fn_name:ident =>
            ($s1:tt, $s2:tt, $s3a:tt $s3b:ident)
            $method:ident ( $(,$args:expr)* ) / $T:ty
            ($e1:tt, $e2:tt, $e3a:tt $e3b:ident)
        ) => {
            #[test]
            fn $fn_name() {
                let mut item = _item!($s1, $s2, $s3a $s3b);
                let _ = item.$method::<$T>($($args),*);
                assert_headers_eq::<StrongType>(&item, &_item!($e1, $e2, $e3a $e3b));
            }
        }
    }

    // Now we get down to what is essentially just a big table of tests, verifying every class of
    // possible behaviour and ensuring that the output is sound.
    //
    // I could try explaining it all in detail, but having considered the matter, I've decided that
    // it's straightforward enough that explanation might be a bit of a waste. I'm sorry if you
    // disagree with me—I can see that it is a bit of a tangle. Stick at it and you should be able
    // to understand it. I'm sorry I caused you trouble.
    // (Fun fact: `1np` is two tokens, `1` and `np`.)

    t!(set_raw_with_raw             => (t, 1, -st) set_raw(,d3raw()) (t, 3, -st))
    t!(set_raw_with_typed           => (f, -, 1st) set_raw(,d3raw()) (t, 3, -st))
    t!(set_raw_with_both            => (t, 1, 3st) set_raw(,d3raw()) (t, 3, -st))
    t!(set_raw_with_invalid_raw     => (f, 1, 3st) set_raw(,d3raw()) (t, 3, -st))

    t!(raw_mut_ref_with_raw         => (t, 1, -st) raw_mut_ref()     (t, 1, -st))
    t!(raw_mut_ref_with_typed       => (f, -, 1st) raw_mut_ref()     (t, 2, -st))
    t!(raw_mut_ref_with_both        => (t, 1, 3st) raw_mut_ref()     (t, 1, -st))
    t!(raw_mut_ref_with_invalid_raw => (f, 1, 3st) raw_mut_ref()     (t, 4, -st))

    t!(raw_ref_with_raw             => (t, 1, -st) raw_ref()         (t, 1, -st))
    t!(raw_ref_with_typed           => (f, -, 1st) raw_ref()         (t, 2, 1st))
    t!(raw_ref_with_both            => (t, 1, 3st) raw_ref()         (t, 1, 3st))
    t!(raw_ref_with_invalid_raw     => (f, 1, 3st) raw_ref()         (t, 4, 3st))

    t!(set_typed_with_raw                      => (t, 1, -st) set_typed(,d3st()) (f, 1, 3st))
    t!(set_typed_with_typed                    => (f, -, 1st) set_typed(,d3st()) (f, -, 3st))
    t!(set_typed_with_other                    => (f, -, 1np) set_typed(,d3st()) (f, -, 3st))
    t!(set_typed_with_other_and_invalid        => (f, 1, 1np) set_typed(,d3st()) (f, 1, 3st))
    t!(set_typed_with_other_and_raw            => (t, 1, 1np) set_typed(,d3st()) (f, 1, 3st))

    t!(typed_mut_ref_with_raw                  => (t, 1, -st) typed_mut_ref()/st (f, 1, 1st))
    t!(typed_mut_ref_with_typed                => (f, -, 1st) typed_mut_ref()/st (f, -, 1st))
    t!(typed_mut_ref_with_other                => (f, -, 3np) typed_mut_ref()/st (f, 4, 4st))
    t!(typed_mut_ref_with_other_and_invalid    => (f, 1, 3np) typed_mut_ref()/st (f, 4, 4st))
    t!(typed_mut_ref_with_other_and_raw        => (t, 1, 3np) typed_mut_ref()/st (f, 1, 1st))
    t!(typed_mut_ref_with_other_np             => (f, -, 3st) typed_mut_ref()/np (f, 4, 3st))
    t!(typed_mut_ref_with_other_and_invalid_np => (f, 1, 3st) typed_mut_ref()/np (f, 4, 3st))
    t!(typed_mut_ref_with_other_and_raw_np     => (t, 1, 3st) typed_mut_ref()/np (f, 1, 3st))

    t!(typed_ref_with_raw                      => (t, 1, -st) typed_ref()/st     (t, 1, 1st))
    t!(typed_ref_with_typed                    => (f, -, 1st) typed_ref()/st     (f, -, 1st))
    t!(typed_ref_with_other                    => (f, -, 3np) typed_ref()/st     (t, 4, 4st))
    t!(typed_ref_with_other_and_invalid        => (f, 1, 3np) typed_ref()/st     (t, 4, 4st))
    t!(typed_ref_with_other_and_raw            => (t, 1, 3np) typed_ref()/st     (t, 1, 1st))
    t!(typed_ref_with_other_np                 => (f, -, 3st) typed_ref()/np     (t, 4, 3st))
    t!(typed_ref_with_other_and_invalid_np     => (f, 1, 3st) typed_ref()/np     (t, 4, 3st))
    t!(typed_ref_with_other_and_raw_np         => (t, 1, 3st) typed_ref()/np     (t, 1, 3st))
}
