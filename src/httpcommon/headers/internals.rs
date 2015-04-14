//! The internals of header representation. That is: `Item`.

use std::borrow::Cow;
use std::ops::Deref;

use mucell::{MuCell, Ref};

use super::{ToHeader, Header, fmt_header};

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
struct Inner {
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
    typed: Option<Box<Header>>,
}

#[derive(PartialEq)]
pub struct Item {
    inner: MuCell<Inner>,
}

impl PartialEq for Inner {
    fn eq(&self, other: &Inner) -> bool {
        match (self, other) {
            (&Inner { raw_valid: true, raw: Some(ref self_v), .. },
             &Inner { raw_valid: true, raw: Some(ref other_v), .. }) => self_v == other_v,

            (&Inner { raw_valid: true, raw: Some(ref self_v), .. },
             &Inner { typed: Some(ref other_h), .. }) => match &self_v[..] {
                [ref self_v_line] => self_v_line == &fmt_header(other_h),
                _ => false,
            },

            (&Inner { typed: Some(ref self_h), .. },
             &Inner { raw_valid: true, raw: Some(ref other_v), .. }) => match &other_v[..] {
                [ref other_v_line] => other_v_line == &fmt_header(self_h),
                _ => false,
            },

            (&Inner { typed: Some(ref self_h), .. },
             &Inner { typed: Some(ref other_h), .. }) => fmt_header(self_h) == fmt_header(other_h),

            _ => unreachable!(),
        }
    }
}

impl Inner {
    fn raw_mut(&mut self, invalidate_typed: bool) -> &mut Vec<Vec<u8>> {
        if self.raw_valid == true {
            // All is good; we'll return the value in good time.
        } else {
            self.raw_valid = true;
            match (&mut self.raw, &mut self.typed) {
                (&mut Some(ref mut raw), &mut Some(ref typed)) => {
                    raw.truncate(1);
                    raw[0] = fmt_header(typed);
                },

                (ref mut raw @ &mut None, &mut Some(ref typed)) => {
                    **raw = Some(vec![fmt_header(typed)]);
                },

                _ => unreachable!(),
            }
        }
        if invalidate_typed {
            self.typed = None;
        }
        self.raw.as_mut().unwrap()
    }

    // Moo!
    fn raw_cow(&self) -> Cow<[Vec<u8>]> {
        if self.raw_valid {
            match self.raw {
                Some(ref vec) => Cow::Borrowed(&vec[..]),
                None => unreachable!(),
            }
        } else {
            match self.typed {
                Some(ref typed) => Cow::Owned(vec![fmt_header(typed)]),
                None => unreachable!(),
            }
        }
    }

    fn typed_mut<H: ToHeader + Header>(&mut self, invalidate_raw: bool) -> Option<&mut H> {
        match self.typed {
            None => {
                debug_assert_eq!(self.raw_valid, true);
                debug_assert!(self.raw.is_some());
                if invalidate_raw {
                    self.raw_valid = false;
                }
                let h: Option<H> = ToHeader::parse_header(self.raw.as_ref().unwrap());
                match h {
                    Some(h) => {
                        self.typed = Some(Box::new(h));
                        Some(unsafe { self.typed.as_mut().unwrap().downcast_mut_unchecked::<H>() })
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
                            raw[0] = fmt_header(h);
                        },
                        None => {
                            self.raw = Some(vec![fmt_header(h)]);
                        },
                    }
                    self.raw_valid = !invalidate_raw;
                } else if invalidate_raw {
                    self.raw_valid = false;
                }
                let otyped: Option<H> = ToHeader::parse_header(self.raw.as_ref().unwrap());
                match otyped {
                    Some(typed) => {
                        *h = Box::new(typed);
                        Some(unsafe { h.downcast_mut_unchecked::<H>() })
                    },
                    None => None,
                }
            },
        }
    }

    // Pass `false` to convert_if_necessary if `typed_mut` was called with the same `H`
    // immediately before; otherwise pass `true`.
    fn typed_cow<H: ToHeader + Header + Clone>(&self, convert_if_necessary: bool) -> Option<Cow<H>> {
        match self.typed {
            Some(ref h) if h.is::<H>() => {
                Some(unsafe { Cow::Borrowed(h.downcast_ref_unchecked::<H>()) })
            },
            _ if convert_if_necessary => {
                ToHeader::parse_header(&*self.raw_cow()).map(|x| Cow::Owned(x))
            },
            _ => None,
        }
    }

}

mucell_ref_type! {
    //#[doc = "TODO"]
    struct RawRef<'a>(Inner),
    impl Deref -> [Vec<u8>],
    data: Cow<'a, [Vec<u8>]> = |x| x.raw_cow()
}

impl<'a> RawRef<'a> {
    /// Extract the owned data.
    ///
    /// Copies the data if it is not already owned.
    pub fn into_owned(self) -> Vec<Vec<u8>> {
        self._data.into_owned()
    }
}

//mucell_ref_type! {
//    //#[doc = "TODO"]
//    struct TypedRef<'a, T: 'static>(Inner),
//    impl Deref -> T,
//    data: Cow<'a, T> = |x| x.typed_cow()
//}

/// An immutable reference to a `MuCell`. Dereference to get at the object.
//$(#[$attr])*
pub struct TypedRef<'a, H: ToHeader + Header + Clone> {
    _parent: Ref<'a, Inner>,
    _data: Cow<'a, H>,
}

impl<'a, H: ToHeader + Header + Clone> TypedRef<'a, H> {
    /// Construct a reference from the cell.
    #[allow(trivial_casts)]  // The `as *const $ty` cast
    fn from(cell: &'a MuCell<Inner>, convert_if_necessary: bool) -> Option<TypedRef<'a, H>> {
        let parent = cell.borrow();
        let inner: &'a Inner = unsafe { &*(&*parent as *const Inner) };
        match inner.typed_cow(convert_if_necessary) {
            Some(data) => Some(TypedRef {
                _parent: parent,
                _data: data,
            }),
            None => None,
        }
    }
}

#[unstable = "trait is not stable"]
impl<'a, H: ToHeader + Header + Clone> Deref for TypedRef<'a, H> {
    type Target = H;
    fn deref<'b>(&'b self) -> &'b H {
        &*self._data
    }
}

impl<'a, H: ToHeader + Header + Clone> TypedRef<'a, H> {
    /// Extract the owned data.
    ///
    /// Copies the data if it is not already owned.
    pub fn into_owned(self) -> H {
        self._data.into_owned()
    }
}

/*************************************************************************************************/

impl Item {
    /// Construct a new Item from a raw representation.
    ///
    /// The vector given MUST contain at least one value, or this will fail.
    pub fn from_raw(raw: Vec<Vec<u8>>) -> Item {
        assert!(raw.len() > 0);
        Item {
            inner: MuCell::new(Inner {
                raw_valid: true,
                raw: Some(raw),
                typed: None,
            }),
        }
    }

    /// Construct a new Item from a typed representation.
    pub fn from_typed<H: Header>(typed: H) -> Item {
        Item {
            inner: MuCell::new(Inner {
                raw_valid: false,
                raw: None,
                typed: Some(Box::new(typed)),
            }),
        }
    }

    /// Get a mutable reference to the raw representation of the header values.
    ///
    /// Because you may modify the raw representation through this mutable reference, calling this
    /// invalidates the typed representation; next time you want to access the value in typed
    /// fashion, it will be parsed from the raw form.
    ///
    /// Only use this if you need to mutate the raw form; if you don't, use `raw`.
    pub fn raw_mut(&mut self) -> &mut Vec<Vec<u8>> {
        self.inner.borrow_mut().raw_mut(true)
    }

    /// Get a reference to the raw representation of the header values.
    ///
    /// If a valid raw representation exists, it will be used, making this a very cheap operation;
    /// if it does not, then the typed representation will be converted to raw form and you will
    /// then get a reference to that. If there are no immutable references already taken, it will
    /// be stored just in case you do it again and you’ll get a reference, otherwise you’ll get the
    /// owned vector. But in summary, it doesn't much matter; you'll get an object that you can
    /// dereference to get your raw reference.
    ///
    /// See also `raw_mut`, if you wish to mutate the raw representation.
    pub fn raw(&self) -> RawRef {
        self.inner.try_mutate(|inner| { let _ = inner.raw_mut(false); });
        RawRef::from(&self.inner)
    }

    /// Set the raw form of the header.
    ///
    /// This invalidates the typed representation.
    pub fn set_raw(&mut self, raw: Vec<Vec<u8>>) {
        let inner = self.inner.borrow_mut();
        inner.raw_valid = true;
        inner.raw = Some(raw);
        inner.typed = None;
    }

    /// Get a mutable reference to the typed representation of the header values.
    ///
    /// Because you may modify the typed representation through this mutable reference, calling
    /// this invalidates the raw representation; next time you want to access the value in raw
    /// fashion, it will be produced from the typed form.
    ///
    /// Only use this if you need to mutate the typed form; if you don't, use `typed`.
    pub fn typed_mut<H: ToHeader + Header>(&mut self) -> Option<&mut H> {
        self.inner.borrow_mut().typed_mut(true)
    }

    /// Get a reference to the typed representation of the header values.
    ///
    /// If a valid typed representation exists, it will be used, making this a very cheap
    /// operation; if it does not, then the raw representation will be converted to typed form and
    /// you will then get a reference to that. If there are no immutable references already taken,
    /// it will be stored just in case you do it again and you’ll get a reference, otherwise you’ll
    /// get the owned vector. In summary, it doesn't much matter; you'll get an object that you
    /// can dereference to get your typed reference.
    ///
    /// See also `typed_mut`, if you wish to mutate the typed representation.
    pub fn typed<H: ToHeader + Header + Clone>(&self) -> Option<TypedRef<H>> {
        let convert_if_necessary = self.inner.try_mutate(|inner| {
            let _ = inner.typed_mut::<H>(false);
        });
        TypedRef::from(&self.inner, convert_if_necessary)
    }

    /// Set the typed form of the header.
    ///
    /// This invalidates the raw representation.
    pub fn set_typed<H: Header>(&mut self, value: H) {
        let inner = self.inner.borrow_mut();
        inner.raw_valid = false;
        inner.typed = Some(Box::new(value));
    }
}

#[cfg(test)]
impl Inner {
    fn assert_invariants(&self) {
        assert!(self.raw.is_some() || !self.raw_valid);
        assert!(self.raw.is_some() || self.typed.is_some());
        assert!(!self.raw_valid || self.raw.as_ref().unwrap().len() > 0);
    }
}

#[cfg(test)]
#[allow(unused_mut)]
mod tests {
    use super::{Item, Inner};
    use super::super::{ToHeader, Header};
    use std::fmt;
    use std::io;
    use mucell::MuCell;

    fn mkitem<H: Header>(raw_valid: bool,
                         raw: Option<Vec<Vec<u8>>>,
                         typed: Option<H>) -> Item {
        let item = Inner {
            raw_valid: raw_valid,
            raw: raw,
            typed: typed.map(|h| { let h: Box<Header> = Box::new(h); h }),
        };
        item.assert_invariants();
        Item { inner: MuCell::new(item) }
    }

    #[derive(PartialEq, Eq, Clone, Debug)]
    struct StrongType(Vec<Vec<u8>>);
    #[allow(non_camel_case_types)]
    type st = StrongType;

    impl ToHeader for StrongType {
        fn parse_header(raw: &[Vec<u8>]) -> Option<StrongType> {
            Some(StrongType(raw.iter().map(|x| x.clone()).collect()))
        }
    }

    impl Header for StrongType {
        fn fmt_header(&self, w: &mut io::Write) -> io::Result<()> {
            let StrongType(ref vec) = *self;
            let mut first = true;
            for field in vec.iter() {
                if !first {
                    try!(w.write_all(b", "));
                }
                try!(w.write_all(field));
                first = false;
            }
            Ok(())
        }
    }

    #[derive(PartialEq, Eq, Clone, Debug)]
    struct NonParsingStrongType(StrongType);
    #[allow(non_camel_case_types)]
    type np = NonParsingStrongType;

    impl ToHeader for NonParsingStrongType {
        fn parse_header(_raw: &[Vec<u8>]) -> Option<NonParsingStrongType> {
            None
        }
    }

    impl Header for NonParsingStrongType {
        fn fmt_header(&self, w: &mut io::Write) -> io::Result<()> {
            let NonParsingStrongType(ref st) = *self;
            st.fmt_header(w)
        }
    }

    fn assert_headers_eq<H: Header + Clone + PartialEq + fmt::Debug>(item: &Item, other: &Item) {
        let item = item.inner.borrow();
        let other = other.inner.borrow();
        item.assert_invariants();
        assert_eq!(item.raw_valid, other.raw_valid);
        assert_eq!(item.raw, other.raw);
        if item.typed.is_some() || other.typed.is_some() {
            let it = item.typed.as_ref().unwrap();
            let ot = other.typed.as_ref().unwrap();
            let ir = it.downcast_ref::<H>().expect("assert_headers_eq: expected Some item, got None");
            let or = ot.downcast_ref::<H>().expect("assert_headers_eq: expected Some other, got None");
            assert_eq!(ir, or);
        }
    }

    // Dummy 1: multiple headers
    fn d1raw() -> Vec<Vec<u8>> {
        vec![b"ab".to_vec(), b"cd".to_vec()]
        //vec![vec![b'a', b'b'], vec![b'c', b'd']]
    }
    fn d1st() -> StrongType { StrongType(d1raw()) }
    fn d1np() -> NonParsingStrongType { NonParsingStrongType(d1st()) }

    // Dummy 2: 1, but merged
    fn d2raw() -> Vec<Vec<u8>> {
        vec![b"ab, cd".to_vec()]
        //vec![vec![b'a', b'b', b',', b' ', b'c', b'd']]
    }
    //fn d2st() -> StrongType { StrongType(d2raw()) }
    //fn d2np() -> NonParsingStrongType { NonParsingStrongType(d2st()) }

    // Dummy 3: multiple headers, different from 1
    fn d3raw() -> Vec<Vec<u8>> {
        vec![b"12".to_vec(), b"34".to_vec()]
        //vec![vec![b'1', b'2'], vec![b'3', b'4']]
    }
    fn d3st() -> StrongType { StrongType(d3raw()) }
    fn d3np() -> NonParsingStrongType { NonParsingStrongType(d3st()) }

    // Dummy 4: 3, but merged
    fn d4raw() -> Vec<Vec<u8>> {
        vec![b"12, 34".to_vec()]
        //vec![vec![b'1', b'2', b',', b' ', b'3', b'4']]
    }
    fn d4st() -> StrongType { StrongType(d4raw()) }
    //fn d4np() -> NonParsingStrongType { NonParsingStrongType(d4st()) }

    #[test]
    #[should_panic]
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
    macro_rules! _bool((t)=>(true);(f)=>(false));
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
            $method:ident ( $(,$args:expr)* ) / $T:ident
            ($e1:tt, $e2:tt, $e3a:tt $e3b:ident)
        ) => {
            #[test]
            fn $fn_name() {
                let mut item = _item!($s1, $s2, $s3a $s3b);
                let _ = item.$method::<$T>($($args),*);
                assert_headers_eq::<StrongType>(&item, &_item!($e1, $e2, $e3a $e3b));
                assert!(item == _item!($e1, $e2, $e3a $e3b));
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

    t!(set_raw_with_raw         => (t, 1, - st) set_raw(,d3raw()) (t, 3, - st));
    t!(set_raw_with_typed       => (f, -, 1 st) set_raw(,d3raw()) (t, 3, - st));
    t!(set_raw_with_both        => (t, 1, 3 st) set_raw(,d3raw()) (t, 3, - st));
    t!(set_raw_with_invalid_raw => (f, 1, 3 st) set_raw(,d3raw()) (t, 3, - st));

    t!(raw_mut_with_raw         => (t, 1, - st) raw_mut()     (t, 1, - st));
    t!(raw_mut_with_typed       => (f, -, 1 st) raw_mut()     (t, 2, - st));
    t!(raw_mut_with_both        => (t, 1, 3 st) raw_mut()     (t, 1, - st));
    t!(raw_mut_with_invalid_raw => (f, 1, 3 st) raw_mut()     (t, 4, - st));

    t!(raw_with_raw             => (t, 1, - st) raw()         (t, 1, - st));
    t!(raw_with_typed           => (f, -, 1 st) raw()         (t, 2, 1 st));
    t!(raw_with_both            => (t, 1, 3 st) raw()         (t, 1, 3 st));
    t!(raw_with_invalid_raw     => (f, 1, 3 st) raw()         (t, 4, 3 st));

    t!(set_typed_with_raw                  => (t, 1, - st) set_typed(,d3st()) (f, 1, 3 st));
    t!(set_typed_with_typed                => (f, -, 1 st) set_typed(,d3st()) (f, -, 3 st));
    t!(set_typed_with_other                => (f, -, 1 np) set_typed(,d3st()) (f, -, 3 st));
    t!(set_typed_with_other_and_invalid    => (f, 1, 1 np) set_typed(,d3st()) (f, 1, 3 st));
    t!(set_typed_with_other_and_raw        => (t, 1, 1 np) set_typed(,d3st()) (f, 1, 3 st));

    t!(typed_mut_with_raw                  => (t, 1, - st) typed_mut()/st (f, 1, 1 st));
    t!(typed_mut_with_typed                => (f, -, 1 st) typed_mut()/st (f, -, 1 st));
    t!(typed_mut_with_other                => (f, -, 3 np) typed_mut()/st (f, 4, 4 st));
    t!(typed_mut_with_other_and_invalid    => (f, 1, 3 np) typed_mut()/st (f, 4, 4 st));
    t!(typed_mut_with_other_and_raw        => (t, 1, 3 np) typed_mut()/st (f, 1, 1 st));
    t!(typed_mut_with_other_np             => (f, -, 3 st) typed_mut()/np (f, 4, 3 st));
    t!(typed_mut_with_other_and_invalid_np => (f, 1, 3 st) typed_mut()/np (f, 4, 3 st));
    t!(typed_mut_with_other_and_raw_np     => (t, 1, 3 st) typed_mut()/np (f, 1, 3 st));

    t!(typed_with_raw                      => (t, 1, - st) typed()/st     (t, 1, 1 st));
    t!(typed_with_typed                    => (f, -, 1 st) typed()/st     (f, -, 1 st));
    t!(typed_with_other                    => (f, -, 3 np) typed()/st     (t, 4, 4 st));
    t!(typed_with_other_and_invalid        => (f, 1, 3 np) typed()/st     (t, 4, 4 st));
    t!(typed_with_other_and_raw            => (t, 1, 3 np) typed()/st     (t, 1, 1 st));
    t!(typed_with_other_np                 => (f, -, 3 st) typed()/np     (t, 4, 3 st));
    t!(typed_with_other_and_invalid_np     => (f, 1, 3 st) typed()/np     (t, 4, 3 st));
    t!(typed_with_other_and_raw_np         => (t, 1, 3 st) typed()/np     (t, 1, 3 st));

    macro_rules! fmtitem {
        ($e:expr) => {{
            let outer = $e;
            let item = outer.inner.borrow();
            let typed = match item.typed {
                Some(ref t) => Some(super::super::fmt_header(t)),
                None => None,
            };
            format!("Item {{ raw_valid: {:?}, raw: {:?}, typed: {:?} }}", item.raw_valid, item.raw,
                    typed)
        }}
    }

    macro_rules! eq {
        (
            $fn_name:ident =>
            ($s1:tt, $s2:tt, $s3a:tt $s3b:ident)
            ($e1:tt, $e2:tt, $e3a:tt $e3b:ident)
        ) => {
            #[test]
            fn $fn_name() {
                //assert_eq!(_item!($s1, $s2, $s3a $s3b), _item!($e1, $e2, $e3a $e3b));
                let a = _item!($s1, $s2, $s3a $s3b);
                let b = _item!($e1, $e2, $e3a $e3b);
                assert!(a == b, "The two are not equal!\n{}\n{}", fmtitem!(a), fmtitem!(b))
            }
        }
    }

    macro_rules! ne {
        (
            $fn_name:ident =>
            ($s1:tt, $s2:tt, $s3a:tt $s3b:ident)
            ($e1:tt, $e2:tt, $e3a:tt $e3b:ident)
        ) => {
            #[test]
            fn $fn_name() {
                let a = _item!($s1, $s2, $s3a $s3b);
                let b = _item!($e1, $e2, $e3a $e3b);
                assert!(a != b, "The two are equal!\n{}\n{}", fmtitem!(a), fmtitem!(b))
            }
        }
    }

    // raw = raw
    eq!(raw_eq_raw_with_different_typed => (t, 2, 1 st) (t, 2, 1 np));
    eq!(raw_eq_raw_with_same_typed      => (t, 2, 1 st) (t, 2, 1 st));
    eq!(raw_eq_raw_with_one_typed       => (t, 2, 1 st) (t, 2, - st));
    eq!(raw_eq_raw_with_neither_typed   => (t, 2, - st) (t, 2, - st));
    ne!(raw_ne_raw_with_different_typed => (t, 2, 1 st) (t, 4, 3 np));
    ne!(raw_ne_raw_with_same_typed      => (t, 2, 1 st) (t, 4, 3 st));
    ne!(raw_ne_raw_with_one_typed       => (t, 2, 1 st) (t, 4, - st));
    ne!(raw_ne_raw_with_neither_typed   => (t, 2, - st) (t, 4, - st));

    // raw = typed
    eq!(raw_eq_typed_with_one_raw         => (t, 2, 3 st) (f, -, 1 st));
    ne!(raw_ne_typed_with_one_raw         => (t, 1, 1 st) (f, -, 1 st));
    eq!(raw_eq_typed_with_invalid_raw     => (t, 2, 3 st) (f, 3, 1 st));
    eq!(raw_eq_typed_with_different_typed => (t, 2, 1 st) (f, -, 1 np));

    // typed = typed
    eq!(typed_eq_typed_with_1              => (f, -, 1 st) (f, -, 1 np));
    eq!(typed_eq_typed_with_2              => (f, -, 1 st) (f, -, 1 st));
    eq!(typed_eq_typed_with_3              => (f, -, 1 st) (f, 2, 1 np));
    eq!(typed_eq_typed_with_4              => (f, -, 1 st) (f, 2, 1 st));
    eq!(typed_eq_typed_with_5              => (f, 2, 1 st) (f, -, 1 np));
    eq!(typed_eq_typed_with_6              => (f, 2, 1 st) (f, -, 1 st));
    eq!(typed_eq_typed_with_7              => (f, 2, 1 st) (f, 2, 1 np));
    eq!(typed_eq_typed_with_8              => (f, 2, 1 st) (f, 2, 1 st));
}
