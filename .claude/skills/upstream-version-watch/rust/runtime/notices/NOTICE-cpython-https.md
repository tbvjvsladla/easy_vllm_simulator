# Attribution and change summary of `src/fetch_https/` (R35: HTTPS plain-str URL fetch, TLS/CONNECT episode)

`src/fetch_https/` extends the direct-HTTP port documented in `NOTICE-cpython-http.md` (read that file first: its table, its change summary, the
PSF License text it points to and the retained Unicode-object header all still apply and are unchanged). This file lists only what the HTTPS
episode adds. The Python sources below were read from the installed copies under `/usr/lib/python3.12/` (CPython 3.12.3, the frozen interpreter
profile) while writing the port.

## Derived code (control flow, limits and message texts follow the Python sources)

| CPython 3.12.3 source (installed copy) | What the port takes from it | Rust module |
|---|---|---|
| `http/client.py` | `HTTPConnection.set_tunnel` (the generated `Host: host:port` header), `_tunnel` (the `CONNECT host:port HTTP/1.1` request lines, the single send, `_read_status` + `_read_headers` of the reply, `OSError("Tunnel connection failed: <code> <message.strip()>")` for any status but 200), `_get_hostport` (default port 443 for a missing or empty port), the connection class defaults `default_port = 443` | `tunnel.rs`, `exchange.rs`, `http_direct/url.rs` |
| `urllib/request.py` | `ProxyHandler.proxy_open` and `_parse_proxy` (scheme/authority/userinfo split, the `https`-request tunnel rule), `getproxies_environment` (lower-case precedence, empty value removes the key, CGI rule for `http` only; generalised from the `http` key to any key), `AbstractHTTPHandler.do_open` (which exceptions are wrapped in `URLError`: the `except OSError` around `h.request`) | `proxy_route.rs`, `http_direct/proxy_scope.rs`, `exchange.rs` |
| `urllib/error.py` | `URLError.__init__`/`__str__` (`args == (reason,)`, attribute `reason`, text `<urlopen error ...>`), the `__context__` it gets from being raised inside the handler | `http_direct/failure.rs` (`url_error`) |

The port restricts these to the captured forms and refuses the rest by name (`out_of_scope`): proxy credentials, a proxy scheme other than `http`,
`no_proxy`, an address that is not `host:port`, redirects over TLS (see `ABI.md`).

## Facts used (observed from the saved original records, not derived from code)

The text and numbers of the three certificate failures were observed from the saved original runs of the frozen interpreter and are reproduced as
facts: error number 20 with `unable to get local issuer certificate`; 64 with `IP address mismatch, certificate is not valid for '<host>'.`;
62 with `Hostname mismatch, certificate is not valid for '<host>'.`; the sentence `[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: <text>
(_ssl.c:1000)`; the `SSLCertVerificationError` attributes `errno`, `strerror`, `filename`, `filename2`, `reason`, `verify_code`, `verify_message`,
`library`. `_ssl.c` and OpenSSL source files were **not** read and no code is derived from them; the text `(_ssl.c:1000)` is the site string of
that interpreter's build, kept so the saved text matches, and says nothing about this port executing `_ssl.c`.

## Libraries used through their public API (no source adapted here)

`rustls` 0.23.46 (MIT OR Apache-2.0 OR ISC; handshake, `ClientConnection`, `StreamOwned`, error types, and `ServerConfig`/`ServerConnection` in the
checks), through the locked version already in the dependency graph; the recipe package's `tls_verify::client_config` and `tls_trust` (adapted
from `ureq` in that package, whose notices are already shipped in `NOTICE.md`). No `ureq` or `rustls` source text was copied into this directory.

## Licenses that apply

The PSF License Version 2 of CPython 3.12.3 (`CPython-3.12.3-LICENSE.txt`, Copyright (c) 2001-2024 Python Software Foundation; All Rights
Reserved) covers the derived code above; its section 2 requires the license text and a brief summary of changes, given here and in
`NOTICE-cpython-http.md`. The Rust port itself is part of this repository and does not relicense CPython.
