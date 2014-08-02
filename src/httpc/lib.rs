//! The Teepee HTTP client.

#![crate_name = "httpc"]
#![comment = "The Teepee HTTP client"]
#![license = "MIT/ASL2"]
#![crate_type = "lib"]

#![doc(html_logo_url = "http://teepee.rs/logo.100.png",
       html_root_url = "http://www.rust-ci.org/teepee/teepee/doc/")]

#![deny(unnecessary_qualification)]
#![deny(non_uppercase_statics)]
#![deny(unnecessary_typecast)]
#![deny(missing_doc)]
//#![deny(unstable)]
#![deny(unused_result)]

extern crate httpcommon;

pub use httpcommon::status;
