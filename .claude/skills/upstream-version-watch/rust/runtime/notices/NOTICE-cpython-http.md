# Attribution and change summary of `src/http_direct/` (R35 stage B1: direct-HTTP plain-str URL fetch)

The modules of `src/http_direct/` are Rust re-expressions of parts of **CPython 3.12.3** (the frozen interpreter profile, Debian/Ubuntu package
`python3.12`; upstream https://github.com/python/cpython tag `v3.12.3`). The Python sources below were read from the installed copies under
`/usr/lib/python3.12/` while writing the port.

| CPython 3.12.3 source (installed copy) | What the port takes from it | Rust module |
|---|---|---|
| `http/client.py` | `_MAXLINE`, `_MAXHEADERS`, `_MIN_READ_BUF_SIZE`; `_read_headers`; `HTTPResponse._read_status`, `begin`, `read`, `_read_chunked`, `_get_chunk_left`, `_read_next_chunk_size`, `_read_and_discard_trailer`, `_safe_read`; `HTTPConnection._get_hostport`, `putrequest`, `putheader` (request head and header order); the exception classes `HTTPException`, `IncompleteRead`, `BadStatusLine`, `LineTooLong`, `UnknownProtocol`, `RemoteDisconnected` (class, message, arguments, attributes) | `response.rs`, `request.rs`, `failure.rs`, `url.rs` |
| `urllib/request.py` | `Request.__init__`/`full_url`/`_parse`, `unwrap`, `_splittype`, `_splithost`, `_splittag`; `OpenerDirector.open/_open/error`; `AbstractHTTPHandler.do_request_/do_open`; `HTTPErrorProcessor.http_response`; `HTTPRedirectHandler` (`http_error_302`, `redirect_request`, `max_repeats`, `max_redirections`, `inf_msg`); `HTTPDefaultErrorHandler`; `getproxies_environment`; `OpenerDirector.addheaders` (User-agent) | `opener.rs`, `url.rs`, `request.rs`, `proxy_scope.rs` |
| `urllib/parse.py` | `urlsplit`/`urlparse`, `_splitnetloc`, `_splitparams`, `urlunparse`/`urlunsplit`, `urljoin`, the `uses_*` scheme lists (restricted to `""`, `http`, `https`, `ftp`), the `quote` rule for `safe=string.punctuation` | `url.rs` |
| `urllib/error.py` | `HTTPError.__init__`/`__str__` (attributes `code`, `msg`/`reason`, `filename`, `url`, headers) | `failure.rs` |
| `email/feedparser.py` | `headerRE`, header-line collection and `_parse_headers` (continuation, "From " and nameless lines, a line without colon ends the block) | `headers.rs` |
| `email/_policybase.py`, `email/message.py` | `Compat32.header_source_parse`; `Message.get`, `__contains__`, `raw_items` | `headers.rs` |
| `Objects/longobject.c`, `Objects/unicodeobject.c` (C, tag `v3.12.3`) | integer literal parsing, the Unicode-to-ASCII numeric transform, the `ValueError` text of a bad literal | `py_int.rs` |

The integer-literal rules of `py_int.rs` were re-derived (repair N1/N2) from the pinned **CPython 3.12.3 C sources** `Objects/longobject.c`
(`PyLong_FromString`, `long_from_string_base`, `_PyLong_FromBytes`, `PyLong_FromUnicodeObject`: the C whitespace set, sign, `0x` prefix, underscore
rules, the 4300-digit limit and the `%.200R` / 200-byte limits of the `ValueError` text) and `Objects/unicodeobject.c`
(`_PyUnicode_TransformDecimalAndSpaceToASCII`). Behaviour of two other C-level pieces was re-implemented from their documented, observable behaviour and checked by
unit tests; their C files were not consulted: `str.split(None, maxsplit)`/`str.strip` (`text.rs`) and `BufferedReader.readline/read` (`wire.rs`).

This is translated, derived code: the control flow, limits, messages and the order of effects follow the Python sources. Python text is not shipped.
Where this port stops short of CPython it refuses by name (`out_of_scope`) instead of approximating; see `ABI.md` and `PORTING.md` of the round.

## Changes against CPython

* Rust types and explicit result values replace exceptions; the exception **objects** CPython would raise are modelled as `PyException`
  (class, module, `str()`, `args`, the attributes the selected contract captures, `__cause__`/`__context__` chain, `HTTPError` headers).
* One connection per request, closed afterwards (`Connection: close`); keep-alive bookkeeping (`will_close`, response reuse) is not carried.
* The socket is a `ureq` 3.5.0 transport chain used as a raw byte pipe (not part of CPython); `socket.timeout` of 20 s is a per-operation timeout.
* `urllib.parse` is carried only for ASCII/Latin-1 text and the schemes above; IPv6 bracket validation and NFKC network-location checks are refused.
* Python frames, audit events and the live file object of `HTTPError` are interpreter observations and are not modelled.

## Licenses that apply (full text shipped next to this file)

1. **PSF License Version 2** (the license of CPython 3.12.3), with the history and the other agreements of the CPython `LICENSE` file:
   `CPython-3.12.3-LICENSE.txt`. Copyright (c) 2001-2024 Python Software Foundation; All Rights Reserved. Section 2 of the PSF License
   requires the license text and a brief summary of the changes, which are given above.
2. The Rust port itself is part of this repository and is distributed under the repository's license; it does not relicense CPython.

The notices of the other translated sources of this crate (`NOTICE-pyhash.md`, `NOTICE.md`) are unchanged.

## Additional upstream file header retained

The numeric transformation in `py_int.rs` is derived from `Objects/unicodeobject.c`. The following is the exact opening header of the pinned CPython 3.12.3 source, including its copyright, permission and disclaimer text.

```text
/*

Unicode implementation based on original code by Fredrik Lundh,
modified by Marc-Andre Lemburg <mal@lemburg.com>.

Major speed upgrades to the method implementations at the Reykjavik
NeedForSpeed sprint, by Fredrik Lundh and Andrew Dalke.

Copyright (c) Corporation for National Research Initiatives.

--------------------------------------------------------------------
The original string type implementation is:

  Copyright (c) 1999 by Secret Labs AB
  Copyright (c) 1999 by Fredrik Lundh

By obtaining, using, and/or copying this software and/or its
associated documentation, you agree that you have read, understood,
and will comply with the following terms and conditions:

Permission to use, copy, modify, and distribute this software and its
associated documentation for any purpose and without fee is hereby
granted, provided that the above copyright notice appears in all
copies, and that both that copyright notice and this permission notice
appear in supporting documentation, and that the name of Secret Labs
AB or the author not be used in advertising or publicity pertaining to
distribution of the software without specific, written prior
permission.

SECRET LABS AB AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO
THIS SOFTWARE, INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS.  IN NO EVENT SHALL SECRET LABS AB OR THE AUTHOR BE LIABLE FOR
ANY SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT
OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
--------------------------------------------------------------------

*/
```
