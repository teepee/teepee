//! Common HTTP functionality for the Teepee project.
//!
//! Anything shared between both client and server belongs in here, but this crate is not expected
//! to be used directly.
//!
//! Any crate using types from this crate should re‐export them. For example, the ``status`` module
//! should be exported in the root of the HTTP client crate ``httpc`` so that people can write
//! ``httpc::status`` instead of ``httpcommon::status``.

#![doc(html_logo_url = "http://teepee.rs/logo.100.png",
       html_root_url = "http://www.rust-ci.org/teepee/teepee/doc/")]

#![feature(concat_idents, unsafe_destructor, plugin, core, slice_patterns)]

#![warn(non_upper_case_globals, missing_docs, unused_results)]

#![plugin(phf_macros)]

extern crate phf;

#[macro_use]
extern crate mucell;

#[macro_use]
extern crate mopa;

pub mod method;
pub mod status;
pub mod headers;
pub mod grammar;
