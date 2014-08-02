//! HTTP request methods.
//!
//! See the `Method` type for all the information your heart desires.
use std::fmt;
use std::mem;

use phf::PhfMap;

use grammar::token;
use grammar::token::Token;

macro_rules! method_enum {
    ($(
        $ident:ident
        $bytes:expr
        $safe:ident
        $idempotent:ident
        #[$doc:meta];
    )*) => {
        static REGISTERED_METHODS: PhfMap<&'static [u8], Method<'static>> = phf_map!(
            $($bytes => $ident,)*
        );

        /// An HTTP method.
        ///
        /// The average developer will be used to only a few methods (`GET`, `POST`, `HEAD`, &c.).
        /// HTTP actually supports arbitrary methods, so long as they are tokens. IANA maintains an
        /// [HTTP Method Registry](http://www.iana.org/assignments/http-methods/http-methods.xhtml)
        /// which is the basis for the variants in this enum. `GET`, for example, is the `Get`
        /// variant here.
        ///
        /// For methods not in the registry there is the `UnregisteredMethod` variant.
        ///
        /// Properties of methods
        /// ---------------------
        ///
        /// It is all too common for developers to go comparing methods by name when what they
        /// *actually* should be doing is comparing the safety or idempotency of a method. This
        /// approach, with the `safe()` and `idempotent()` methods, should be preferred.
        ///
        /// The ability to cache a request of a given method is the third common property (besides
        /// safety and idempotency) identified in RFC 7231; it, however, is less well-defined, and
        /// so there is no explicit API to determine it at present.
        ///
        /// All three of these common properties are described in some detail in [RFC 7231, section
        /// 4.2](https://tools.ietf.org/html/rfc7231#section-4.2).
        ///
        /// Unregistered methods will default to not being safe and not being idempotent, but may
        /// be altered after creation if desired.
        #[deriving(Clone, Hash)]
        pub enum Method<'a> {
            $(#[$doc] $ident,)*
            /// A method not in the IANA HTTP method registry.
            UnregisteredMethod {
                /// The method name.
                pub name: Token<'a>,
                /// Whether the method is safe or not.
                pub safe: bool,
                /// Whether the method is idempotent or not.
                pub idempotent: bool,
            },
        }

        impl<'a> Method<'a> {
            /// Create a `Method` instance from a token.
            ///
            /// Where possible, this will use the fancy variants like `Get`:
            ///
            /// ```rust
            /// # use httpcommon::grammar::token::Token;
            /// # use httpcommon::method::{Method, Get};
            /// let token = Token::from_slice(b"GET").unwrap();
            /// assert_eq!(Method::from_token(token), Get);
            /// ```
            ///
            /// But for a token that does not refer to a registered method, it will create an
            /// `UnregisteredMethod` with `safe` and `idempotent` both set to `false`:
            ///
            /// ```rust
            /// # use httpcommon::grammar::token::Token;
            /// # use httpcommon::method::{Method, UnregisteredMethod};
            /// let token = Token::from_slice(b"PANIC").unwrap();
            /// let panic = UnregisteredMethod {
            ///     name: token.clone(),
            ///     safe: false,
            ///     idempotent: false,
            /// };
            /// assert_eq!(Method::from_token(token), panic);
            /// ```
            ///
            /// If you happen to know about the token and that it is not a registered method,
            /// you may also choose to just construct an `UnregisteredMethod` directly, with
            /// appropriate values for `safe` and `idempotent`. If doing this, bear in mind that if
            /// a method name is registered with IANA, when it is added to this library, it will
            /// all of a sudden *stop* returning `UnregisteredMethod`, and so your code could
            /// conceivably break. In the example above, for example, it might start returning
            /// a new variant `Panic` instead of an `UnregisteredMethod`.
            ///
            /// See also `registered_from_token`.
            pub fn from_token<'a>(token: Token<'a>) -> Method<'a> {
                match REGISTERED_METHODS.find_equiv(&token.as_bytes()) {
                    Some(registered_token) => registered_token.clone(),
                    None => UnregisteredMethod {
                        name: token,
                        safe: false,
                        idempotent: false,
                    },
                }
            }

            /// Produce a registered `method` from a token.
            ///
            /// ```rust
            /// # use httpcommon::grammar::token::Token;
            /// # use httpcommon::method::{Method, Get};
            /// let token = Token::from_slice(b"GET").unwrap();
            /// assert_eq!(Method::registered_from_token(token), Some(Get));
            /// ```
            ///
            /// If the token does not refer to a registered method, this will produce `None`.
            ///
            /// ```rust
            /// # use httpcommon::grammar::token::Token;
            /// # use httpcommon::method::Method;
            /// let token = Token::from_slice(b"PANIC").unwrap();
            /// assert_eq!(Method::registered_from_token(token), None);
            /// ```
            ///
            /// This will never produce the `UnregisteredMethod` variant.
            ///
            /// Bear in mind that where in one release this may return `None` for a given token, in
            /// the next release it may return `Some` for the same token, if that token corresponds
            /// to a new entry in the IANA HTTP Method Registry.
            ///
            /// See also `from_token`.
            pub fn registered_from_token(token: Token) -> Option<Method<'static>> {
                REGISTERED_METHODS.find_equiv(&token.as_bytes()).map(|t| t.clone())
            }

            /// Change a slice-token-based method to use an owned-token.
            ///
            /// This may incur an allocation, but fixes the lifetime up.
            #[inline]
            pub fn into_owned(self) -> Method<'static> {
                match self {
                    UnregisteredMethod { name, safe, idempotent } =>
                        UnregisteredMethod {
                            name: name.into_owned(),
                            safe: safe,
                            idempotent: idempotent,
                        },
                    // Let’s fix the lifetime issue in one fell swoop. This is entirely reasonable,
                    // for they are all simple discriminants. I just don’t want to write
                    // `$($ident => $ident,)*`, if it’s all the same to you.
                    registered_method => unsafe { mem::transmute(registered_method) },
                }
            }

            /// Retrieve the method name.
            ///
            /// Where feasible you should avoid using this; compare against known values instead,
            /// or use `safe()` and `idempotent()` where feasible.
            pub fn name<'b>(&'b self) -> Token<'b> {
                match *self {
                    $($ident => token::Slice { _bytes: $bytes },)*
                    UnregisteredMethod { ref name, .. } => name.slice(),
                }
            }

            /// Whether the method is safe.
            ///
            /// Here is the explanation offered by [RFC 7231, section 4.2.1 Safe
            /// Methods](https://tools.ietf.org/html/rfc7231#section-4.2.1) of what this means:
            ///
            /// > Request methods are considered "safe" if their defined semantics are
            /// > essentially read-only; i.e., the client does not request, and does
            /// > not expect, any state change on the origin server as a result of
            /// > applying a safe method to a target resource.  Likewise, reasonable
            /// > use of a safe method is not expected to cause any harm, loss of
            /// > property, or unusual burden on the origin server.
            /// >
            /// > This definition of safe methods does not prevent an implementation
            /// > from including behavior that is potentially harmful, that is not
            /// > entirely read-only, or that causes side effects while invoking a safe
            /// > method.  What is important, however, is that the client did not
            /// > request that additional behavior and cannot be held accountable for
            /// > it.  For example, most servers append request information to access
            /// > log files at the completion of every response, regardless of the
            /// > method, and that is considered safe even though the log storage might
            /// > become full and crash the server.  Likewise, a safe request initiated
            /// > by selecting an advertisement on the Web will often have the side
            /// > effect of charging an advertising account.
            /// >
            /// > Of the request methods defined by this specification, the GET, HEAD,
            /// > OPTIONS, and TRACE methods are defined to be safe.
            /// >
            /// > The purpose of distinguishing between safe and unsafe methods is to
            /// > allow automated retrieval processes (spiders) and cache performance
            /// > optimization (pre-fetching) to work without fear of causing harm.  In
            /// > addition, it allows a user agent to apply appropriate constraints on
            /// > the automated use of unsafe methods when processing potentially
            /// > untrusted content.
            /// >
            /// > A user agent SHOULD distinguish between safe and unsafe methods when
            /// > presenting potential actions to a user, such that the user can be
            /// > made aware of an unsafe action before it is requested.
            /// >
            /// > When a resource is constructed such that parameters within the
            /// > effective request URI have the effect of selecting an action, it is
            /// > the resource owner's responsibility to ensure that the action is
            /// > consistent with the request method semantics.  For example, it is
            /// > common for Web-based content editing software to use actions within
            /// > query parameters, such as "page?do=delete".  If the purpose of such a
            /// > resource is to perform an unsafe action, then the resource owner MUST
            /// > disable or disallow that action when it is accessed using a safe
            /// > request method.  Failure to do so will result in unfortunate side
            /// > effects when automated processes perform a GET on every URI reference
            /// > for the sake of link maintenance, pre-fetching, building a search
            /// > index, etc.
            ///
            /// For registered methods, the data from the IANA HTTP Method Registry is all loaded
            /// correctly. Unregistered methods default to claiming that they are not safe.
            pub fn safe(&self) -> bool {
                match *self {
                    $($ident => $safe,)*
                    UnregisteredMethod { safe, .. } => safe,
                }
            }

            /// Whether the method is idempotent.
            ///
            /// Here is the explanation offered by [RFC 7231, section 4.2.2 Idempotent
            /// Methods](https://tools.ietf.org/html/rfc7231#section-4.2.2) of what this means:
            ///
            /// > A request method is considered "idempotent" if the intended effect on
            /// > the server of multiple identical requests with that method is the
            /// > same as the effect for a single such request.  Of the request methods
            /// > defined by this specification, PUT, DELETE, and safe request methods
            /// > are idempotent.
            /// >
            /// > Like the definition of safe, the idempotent property only applies to
            /// > what has been requested by the user; a server is free to log each
            /// > request separately, retain a revision control history, or implement
            /// > other non-idempotent side effects for each idempotent request.
            /// >
            /// > Idempotent methods are distinguished because the request can be
            /// > repeated automatically if a communication failure occurs before the
            /// > client is able to read the server's response.  For example, if a
            /// > client sends a PUT request and the underlying connection is closed
            /// > before any response is received, then the client can establish a new
            /// > connection and retry the idempotent request.  It knows that repeating
            /// > the request will have the same intended effect, even if the original
            /// > request succeeded, though the response might differ.
            ///
            /// For registered methods, the data from the IANA HTTP Method Registry is all loaded
            /// correctly. Unregistered methods default to claiming that they are not idempotent.
            pub fn idempotent(&self) -> bool {
                match *self {
                    $($ident => $idempotent,)*
                    UnregisteredMethod { idempotent, .. } => idempotent,
                }
            }
        }

        impl<'a> PartialOrd for Method<'a> {
            #[inline]
            fn partial_cmp(&self, other: &Method<'a>) -> Option<Ordering> {
                (self.name(), self.safe(), self.idempotent()).partial_cmp(
                    &(other.name(), other.safe(), other.idempotent()))
            }
        }

        impl<'a> Ord for Method<'a> {
            #[inline]
            fn cmp(&self, other: &Method<'a>) -> Ordering {
                (self.name(), self.safe(), self.idempotent()).cmp(
                    &(other.name(), other.safe(), other.idempotent()))
            }
        }

        impl<'a> PartialEq for Method<'a> {
            #[inline]
            fn eq(&self, other: &Method<'a>) -> bool {
                match (self, other) {
                    (_, &UnregisteredMethod { .. }) |
                    (&UnregisteredMethod { .. }, _) => {
                        self.name() == other.name() &&
                        self.safe() == other.safe() &&
                        self.idempotent() == other.idempotent()
                    },
                    $((&$ident, &$ident) => true,)*
                    _ => false,
                }
            }
        }

        impl<'a> Eq for Method<'a> { }

        impl<'a> Collection for Method<'a> {
            #[inline]
            fn len(&self) -> uint {
                self.name().len()
            }
        }

        impl<'a> fmt::Show for Method<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write(self.name().as_bytes())
            }
        }
    }
}

// FIXME(Chris): this is pretty absurd just at present. Especially the duplication between string
// and bytes, for PHF. Ideally I should come up with a way to fix it all up so that all we need is
// the variant name, bytes, safe, idempotent and RFC numbers/sections, probably by a procedural
// macro. Making phf cope with byte literals would help too, and sfackler has said he would accept
// a change from PhfMap<V> to PhfMap<K, V>.
method_enum! {
    // Variant name   method name bytes    safe  idempotent
    Acl               b"ACL"               false true  #[doc = "`ACL`, defined in [RFC 3744, section 8.1](https://tools.ietf.org/html/rfc3744#section-8.1). Not safe, but idempotent."];
    BaselineControl   b"BASELINE-CONTROL"  false true  #[doc = "`BASELINE-CONTROL`, defined in [RFC 3253, section 12.6](https://tools.ietf.org/html/rfc3253#section-12.6). Not safe, but idempotent."];
    Bind              b"BIND"              false true  #[doc = "`BIND`, defined in [RFC 5842, section 4](https://tools.ietf.org/html/rfc5842#section-4). Not safe, but idempotent."];
    Checkin           b"CHECKIN"           false true  #[doc = "`CHECKIN`, defined in [RFC 3253, section 4.4](https://tools.ietf.org/html/rfc3253#section-4.4) and [section 9.4](https://tools.ietf.org/html/rfc3253#section-9.4). Not safe, but idempotent."];
    Checkout          b"CHECKOUT"          false true  #[doc = "`CHECKOUT`, defined in [RFC 3253, section 4.3](https://tools.ietf.org/html/rfc3253#section-4.3) and [section 8.8](https://tools.ietf.org/html/rfc3253#section-8.8). Not safe, but idempotent."];
    Connect           b"CONNECT"           false false #[doc = "`CONNECT`, defined in [RFC 7231, section 4.3.6](https://tools.ietf.org/html/rfc7231#section-4.3.6). Not safe and not idempotent."];
    Copy              b"COPY"              false true  #[doc = "`COPY`, defined in [RFC 4918, section 9.8](https://tools.ietf.org/html/rfc4918#section-9.8). Not safe, but idempotent."];
    Delete            b"DELETE"            false true  #[doc = "`DELETE`, defined in [RFC 7231, section 4.3.5](https://tools.ietf.org/html/rfc7231#section-4.3.5). Not safe, but idempotent."];
    Get               b"GET"               true  true  #[doc = "`GET`, defined in [RFC 7231, section 4.3.1](https://tools.ietf.org/html/rfc7231#section-4.3.1). Safe and idempotent."];
    Head              b"HEAD"              true  true  #[doc = "`HEAD`, defined in [RFC 7231, section 4.3.2](https://tools.ietf.org/html/rfc7231#section-4.3.2). Safe and idempotent."];
    Label             b"LABEL"             false true  #[doc = "`LABEL`, defined in [RFC 3253, section 8.2](https://tools.ietf.org/html/rfc3253#section-8.2). Not safe, but idempotent."];
    Link              b"LINK"              false true  #[doc = "`LINK`, defined in [RFC 2068, section 19.6.1.2](https://tools.ietf.org/html/rfc2068#section-19.6.1.2). Not safe, but idempotent."];
    Lock              b"LOCK"              false false #[doc = "`LOCK`, defined in [RFC 4918, section 9.10](https://tools.ietf.org/html/rfc4918#section-9.10). Not safe and not idempotent."];
    Merge             b"MERGE"             false true  #[doc = "`MERGE`, defined in [RFC 3253, section 11.2](https://tools.ietf.org/html/rfc3253#section-11.2). Not safe, but idempotent."];
    MkActivity        b"MKACTIVITY"        false true  #[doc = "`MKACTIVITY`, defined in [RFC 3253, section 13.5](https://tools.ietf.org/html/rfc3253#section-13.5). Not safe, but idempotent."];
    MkCalendar        b"MKCALENDAR"        false true  #[doc = "`MKCALENDAR`, defined in [RFC 4791, section 5.3.1](https://tools.ietf.org/html/rfc4791#section-5.3.1). Not safe, but idempotent."];
    MkCol             b"MKCOL"             false true  #[doc = "`MKCOL`, defined in [RFC 4918, section 9.3](https://tools.ietf.org/html/rfc4918#section-9.3). Not safe, but idempotent."];
    MkRedirectRef     b"MKREDIRECTREF"     false true  #[doc = "`MKREDIRECTREF`, defined in [RFC 4437, section 6](https://tools.ietf.org/html/rfc4437#section-6). Not safe, but idempotent."];
    MkWorkspace       b"MKWORKSPACE"       false true  #[doc = "`MKWORKSPACE`, defined in [RFC 3253, section 6.3](https://tools.ietf.org/html/rfc3253#section-6.3). Not safe, but idempotent."];
    Move              b"MOVE"              false true  #[doc = "`MOVE`, defined in [RFC 4918, section 9.9](https://tools.ietf.org/html/rfc4918#section-9.9). Not safe, but idempotent."];
    Options           b"OPTIONS"           true  true  #[doc = "`OPTIONS`, defined in [RFC 7231, section 4.3.7](https://tools.ietf.org/html/rfc7231#section-4.3.7). Safe and idempotent."];
    OrderPatch        b"ORDERPATCH"        false true  #[doc = "`ORDERPATCH`, defined in [RFC 3648, section 7](https://tools.ietf.org/html/rfc3648#section-7). Not safe, but idempotent."];
    Patch             b"PATCH"             false false #[doc = "`PATCH`, defined in [RFC 5789, section 2](https://tools.ietf.org/html/rfc5789#section-2). Not safe and not idempotent."];
    Post              b"POST"              false false #[doc = "`POST`, defined in [RFC 7231, section 4.3.3](https://tools.ietf.org/html/rfc7231#section-4.3.3). Not safe and not idempotent."];
    PropFind          b"PROPFIND"          true  true  #[doc = "`PROPFIND`, defined in [RFC 4918, section 9.1](https://tools.ietf.org/html/rfc4918#section-9.1). Safe and idempotent."];
    PropPatch         b"PROPPATCH"         false true  #[doc = "`PROPPATCH`, defined in [RFC 4918, section 9.2](https://tools.ietf.org/html/rfc4918#section-9.2). Not safe, but idempotent."];
    Put               b"PUT"               false true  #[doc = "`PUT`, defined in [RFC 7231, section 4.3.4](https://tools.ietf.org/html/rfc7231#section-4.3.4). Not safe, but idempotent."];
    Rebind            b"REBIND"            false true  #[doc = "`REBIND`, defined in [RFC 5842, section 6](https://tools.ietf.org/html/rfc5842#section-6). Not safe, but idempotent."];
    Report            b"REPORT"            true  true  #[doc = "`REPORT`, defined in [RFC 3253, section 3.6](https://tools.ietf.org/html/rfc3253#section-3.6). Safe and idempotent."];
    Search            b"SEARCH"            true  true  #[doc = "`SEARCH`, defined in [RFC 5323, section 2](https://tools.ietf.org/html/rfc5323#section-2). Safe and idempotent."];
    Trace             b"TRACE"             true  true  #[doc = "`TRACE`, defined in [RFC 7231, section 4.3.8](https://tools.ietf.org/html/rfc7231#section-4.3.8). Safe and idempotent."];
    Unbind            b"UNBIND"            false true  #[doc = "`UNBIND`, defined in [RFC 5842, section 5](https://tools.ietf.org/html/rfc5842#section-5). Not safe, but idempotent."];
    Uncheckout        b"UNCHECKOUT"        false true  #[doc = "`UNCHECKOUT`, defined in [RFC 3253, section 4.5](https://tools.ietf.org/html/rfc3253#section-4.5). Not safe, but idempotent."];
    Unlink            b"UNLINK"            false true  #[doc = "`UNLINK`, defined in [RFC 2068, section 19.6.1.3](https://tools.ietf.org/html/rfc2068#section-19.6.1.3). Not safe, but idempotent."];
    Unlock            b"UNLOCK"            false true  #[doc = "`UNLOCK`, defined in [RFC 4918, section 9.11](https://tools.ietf.org/html/rfc4918#section-9.11). Not safe, but idempotent."];
    Update            b"UPDATE"            false true  #[doc = "`UPDATE`, defined in [RFC 3253, section 7.1](https://tools.ietf.org/html/rfc3253#section-7.1). Not safe, but idempotent."];
    UpdateRedirectRef b"UPDATEREDIRECTREF" false true  #[doc = "`UPDATEREDIRECTREF`, defined in [RFC 4437, section 7](https://tools.ietf.org/html/rfc4437#section-7). Not safe, but idempotent."];
    VersionControl    b"VERSION-CONTROL"   false true  #[doc = "`VERSION-CONTROL`, defined in [RFC 3253, section 3.5](https://tools.ietf.org/html/rfc3253#section-3.5). Not safe, but idempotent."];
}
