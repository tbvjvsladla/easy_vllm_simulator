//! Which way an `https` request leaves: directly, or through an HTTP proxy with `CONNECT`.
//!
//! `urllib.request.ProxyHandler` reads the `https` entry of `getproxies_environment()`. When it is absent the request is direct. When it is
//! present, `proxy_bypass` consults the `no` entry and `_parse_proxy` splits the value; for an `https` request the connection is then a
//! `CONNECT` tunnel to the `host:port` of that value (the proxy's own scheme does not change that, and the proxy leg is always plain TCP).
//!
//! Only the form that was captured is implemented: `http://host:port` (or `host:port`) without credentials, no `no_proxy`. Every other
//! relevant setting is refused by name; a proxy that is configured is never bypassed.

use crate::http_direct::failure::{out_of_scope, Fetched};
use crate::http_direct::ProxyEnvironment;

/// How to reach the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Direct,
    /// Connect to this proxy and ask it for a tunnel to the server.
    Tunnel(ProxyAddress),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyAddress {
    pub host: String,
    pub port: u16,
}

/// The route the environment selects for an `https` request.
pub fn choose(proxies: &ProxyEnvironment) -> Fetched<Route> {
    let Some(raw) = proxies.proxy_value("https") else {
        return Ok(Route::Direct);
    };
    if proxies.proxy_value("no").is_some() {
        return Err(out_of_scope(
            "no_proxy_environment",
            "an https proxy and no_proxy are both set; urllib would test the host against no_proxy, which is not implemented",
        ));
    }
    let value = std::str::from_utf8(raw).map_err(|_| {
        out_of_scope(
            "proxy_environment",
            "the https proxy variable holds bytes that are not UTF-8; it is neither used nor ignored",
        )
    })?;
    parse_proxy(value).map(Route::Tunnel)
}

/// `urllib.request._parse_proxy` restricted to what was captured.
fn parse_proxy(value: &str) -> Fetched<ProxyAddress> {
    let authority = proxy_authority(value)?;
    if authority.contains('@') {
        return Err(out_of_scope(
            "proxy_credentials",
            "an https proxy with user information needs a Proxy-Authorization header on the CONNECT; its wire form was not captured",
        ));
    }
    let malformed = || {
        out_of_scope(
            "proxy_hostport",
            "the https proxy must be written as host:port with a DNS name or IPv4 address and an explicit port",
        )
    };
    let (host, port) = authority.rsplit_once(':').ok_or_else(malformed)?;
    let host_is_plain = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let port_is_plain = (1..=5).contains(&port.len()) && port.chars().all(|c| c.is_ascii_digit());
    let port: u16 = port
        .parse()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(malformed)?;
    if !host_is_plain || !port_is_plain {
        return Err(malformed());
    }
    Ok(ProxyAddress {
        host: host.to_string(),
        port,
    })
}

/// The authority part: what `_parse_proxy` leaves of the value after the optional `scheme://` and before the first `/`.
fn proxy_authority(value: &str) -> Fetched<&str> {
    // `_splittype`: `([^/:]+):(.*)`
    let after_scheme = value
        .split_once(':')
        .filter(|(scheme, _)| !scheme.is_empty() && !scheme.contains('/'));
    let Some((scheme, rest)) = after_scheme else {
        return Ok(value);
    };
    if !rest.starts_with('/') {
        // `host:port`: the whole value is the authority.
        return Ok(value);
    }
    if !rest.starts_with("//") {
        return Err(out_of_scope(
            "proxy_environment",
            "an https proxy URL with no authority makes urllib raise ValueError; not captured",
        ));
    }
    if !scheme.eq_ignore_ascii_case("http") {
        return Err(out_of_scope(
            "proxy_scheme",
            format!(
                "the https proxy scheme '{scheme}' is not http; only an http proxy was captured"
            ),
        ));
    }
    let search_from = rest.find('@').unwrap_or(2);
    let end = rest[search_from..]
        .find('/')
        .map_or(rest.len(), |at| search_from + at);
    Ok(&rest[2..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::failure::FetchFailure;

    fn environment(pairs: &[(&str, &str)]) -> ProxyEnvironment {
        ProxyEnvironment::from_pairs(
            pairs
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string())),
        )
    }

    fn route(pairs: &[(&str, &str)]) -> Fetched<Route> {
        choose(&environment(pairs))
    }

    fn refused(pairs: &[(&str, &str)]) -> &'static str {
        match route(pairs) {
            Err(FetchFailure::OutOfScope(refusal)) => refusal.boundary,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    fn tunnel(host: &str, port: u16) -> Route {
        Route::Tunnel(ProxyAddress {
            host: host.to_string(),
            port,
        })
    }

    #[test]
    fn no_https_proxy_means_direct_whatever_the_other_proxies_say() {
        assert_eq!(route(&[]).unwrap(), Route::Direct);
        assert_eq!(
            route(&[
                ("http_proxy", "http://p:1"),
                ("no_proxy", "x"),
                ("all_proxy", "y")
            ])
            .unwrap(),
            Route::Direct
        );
        assert_eq!(route(&[("https_proxy", "")]).unwrap(), Route::Direct);
    }

    #[test]
    fn the_accepted_proxy_forms_select_a_tunnel() {
        assert_eq!(
            route(&[("https_proxy", "http://127.0.0.1:46655")]).unwrap(),
            tunnel("127.0.0.1", 46655)
        );
        assert_eq!(
            route(&[("HTTPS_PROXY", "HTTP://proxy.example:3128/")]).unwrap(),
            tunnel("proxy.example", 3128)
        );
        assert_eq!(
            route(&[("https_proxy", "10.0.0.2:8080")]).unwrap(),
            tunnel("10.0.0.2", 8080)
        );
        // the lower-case variable wins over the upper-case one, an empty lower-case one switches the proxy off
        assert_eq!(
            route(&[("HTTPS_PROXY", "http://a:1"), ("https_proxy", "http://b:2")]).unwrap(),
            tunnel("b", 2)
        );
        assert_eq!(
            route(&[("HTTPS_PROXY", "http://a:1"), ("https_proxy", "")]).unwrap(),
            Route::Direct
        );
    }

    #[test]
    fn relevant_unknown_settings_are_refused_by_name() {
        assert_eq!(
            refused(&[("https_proxy", "http://u:p@h:1")]),
            "proxy_credentials"
        );
        assert_eq!(
            refused(&[("https_proxy", "http://h:1/path@x")]),
            "proxy_credentials"
        );
        assert_eq!(refused(&[("https_proxy", "socks5://h:1")]), "proxy_scheme");
        assert_eq!(refused(&[("https_proxy", "https://h:1")]), "proxy_scheme");
        assert_eq!(
            refused(&[("https_proxy", "http:/h:1")]),
            "proxy_environment"
        );
        assert_eq!(
            refused(&[("https_proxy", "http://h:1"), ("no_proxy", "localhost")]),
            "no_proxy_environment"
        );
        for malformed in [
            "http://h",
            "h",
            "http://h:0",
            "http://h:65536",
            "http://:80",
            "http://[::1]:1",
            "http://h x:1",
        ] {
            assert_eq!(
                refused(&[("https_proxy", malformed)]),
                "proxy_hostport",
                "{malformed}"
            );
        }
    }

    #[test]
    fn a_proxy_value_that_is_not_utf8_is_never_ignored() {
        let environment =
            ProxyEnvironment::from_bytes([(b"https_proxy".to_vec(), vec![0xff, b':', b'1'])]);
        assert!(matches!(
            choose(&environment),
            Err(FetchFailure::OutOfScope(refusal)) if refusal.boundary == "proxy_environment"
        ));
        // an unrelated variable of any encoding does not matter
        let unrelated = ProxyEnvironment::from_bytes([(b"X\xff_proxy_not".to_vec(), vec![0xfe])]);
        assert_eq!(choose(&unrelated).unwrap(), Route::Direct);
    }
}
