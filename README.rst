=============================
Teepee: the Rust HTTP toolkit
=============================

.. image:: https://travis-ci.org/teepee/teepee.png?branch=master
   :target: https://travis-ci.org/teepee/teepee

Status: stalled and designing
=============================

Things happened, things didn’t happen, Hyper got started by others instead.
Something is still slowly happening with this, but don’t try doing anything
with it.

**The remainder of this document is out of date.**
Use [Hyper](https://github.com/hyperium/hyper) instead for now, too.

----

Status: in design
=================

Teepee is still mostly in design. Most of the information about it is available
`on Chris Morgan’s blog`_.

In the mean time, if you want HTTP, please use rust-http_ (by the same author,
but not designed deliberately); it’s in maintenance mode, so new things won’t
be being added to it—but see that as a good thing and not a bad thing! :P

The crates
==========

The Teepee project is comprised of various crates. Here are what there is at
present, though very little is actually implemented yet.

``httpc``: HTTP client
----------------------

Everything client-specific, from low level to high level interface.

``httpd``: HTTP server
----------------------

Everything server-specific, from low level to high level interface.

``httpcommon``: common HTTP functionality
-----------------------------------------

Anything shared between both client and server belongs in here, but this crate
is not expected to be used directly.

Any crate using types from this crate should re‐export them. For example, the
``status`` module should be exported in the root of the HTTP client crate
``httpc`` so that people can write ``httpc::status`` instead of
``httpcommon::status``.

Author
======

`Chris Morgan`_ (chris-morgan_) is the primary author and maintainer of Teepee.

License
=======

This library is distributed under similar terms to Rust: dual licensed under
the MIT license and the Apache license (version 2.0).

See LICENSE-APACHE, LICENSE-MIT, and COPYRIGHT for details.

.. _on Chris Morgan’s blog: http://chrismorgan.info/blog/tags/teepee.html
.. _rust-http: https://github.com/chris-morgan/rust-http
.. _Chris Morgan: http://chrismorgan.info/
.. _chris-morgan: https://github.com/chris-morgan
