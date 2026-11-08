//! The proxy boundary of the direct-HTTP scope.
//!
//! `urllib.request.build_opener()` installs a `ProxyHandler` that reads `<scheme>_proxy` variables (`getproxies_environment`). This
//! component connects directly and does not implement proxies, so it refuses to run when urllib would have an `http` proxy configured,
//! instead of silently bypassing it. The refusal is conservative: `no_proxy` is not interpreted (a configured `http` proxy refuses
//! even when `no_proxy` would have exempted the host). An `http` proxy variable counts when its value is non-empty, whatever bytes it holds.
//! The HTTPS fetch reads the `https` and `no` keys of the same environment through [`ProxyEnvironment::proxy_value`].

/// The suffix `getproxies_environment` looks for in a variable name (`name[-6] == "_"` and `name[-5:].lower() == "proxy"`).
const PROXY_SUFFIX: &[u8] = b"_proxy";
/// `urllib.request.getproxies_environment`: variable whose presence turns the upper-case `HTTP_PROXY` off (the CGI rule).
const CGI_MARKER: &[u8] = b"REQUEST_METHOD";

/// The variables of an environment that the `http` proxy decision can depend on, kept as the raw bytes of the operating system.
///
/// Python's `os.environ` decodes every name and value with `surrogateescape`, and `getproxies_environment` looks at a value only after the
/// name ends in `_proxy` (any case). So an unrelated variable, however it is encoded, never matters, and the value of a proxy variable matters
/// only by being empty or not. Bytes are therefore enough, and nothing here requires a name or value to be valid Unicode.
#[derive(Clone, Debug, Default)]
pub struct ProxyEnvironment {
    variables: Vec<(Vec<u8>, Vec<u8>)>,
}

/// `len(name) > 5 and name[-6] == "_" and name[-5:].lower() == "proxy"`.
fn names_a_proxy(name: &[u8]) -> bool {
    name.len() >= PROXY_SUFFIX.len()
        && name[name.len() - PROXY_SUFFIX.len()..].eq_ignore_ascii_case(PROXY_SUFFIX)
        && name[name.len() - PROXY_SUFFIX.len()] == b'_'
}

/// `name[:-6].lower() == kind` for a name that [`names_a_proxy`] (`kind` is lower-case).
fn is_kind(name: &[u8], kind: &[u8]) -> bool {
    name[..name.len() - PROXY_SUFFIX.len()].eq_ignore_ascii_case(kind)
}

impl ProxyEnvironment {
    /// An environment made of the given pairs (text names and values).
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> ProxyEnvironment {
        ProxyEnvironment::from_bytes(
            pairs
                .into_iter()
                .map(|(name, value)| (name.into_bytes(), value.into_bytes())),
        )
    }

    /// An environment made of raw name/value bytes. Only the entries the proxy decision can read are kept.
    pub fn from_bytes(pairs: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>) -> ProxyEnvironment {
        ProxyEnvironment {
            variables: pairs
                .into_iter()
                .filter(|(name, _)| names_a_proxy(name) || name == CGI_MARKER)
                .collect(),
        }
    }

    /// The variables of this process. Entries that are not valid Unicode are read as bytes (`vars_os`), never panicking.
    pub fn from_process() -> ProxyEnvironment {
        ProxyEnvironment::from_bytes(
            std::env::vars_os().map(|(name, value)| (os_bytes(name), os_bytes(value))),
        )
    }

    /// `getproxies_environment().get(kind)` for a lower-case `kind` (`http`, `https`, `no`): the value of the variable that wins, with the
    /// lower-case precedence, the empty-value rule and, for `http`, the CGI rule for `HTTP_PROXY`; `None` when the key is absent.
    pub fn proxy_value(&self, kind: &str) -> Option<&[u8]> {
        let kind = kind.as_bytes();
        let mut chosen: Option<&[u8]> = None;
        for (name, value) in &self.variables {
            if names_a_proxy(name) && !value.is_empty() && is_kind(name, kind) {
                chosen = Some(value);
            }
        }
        if kind == b"http" && self.variables.iter().any(|(name, _)| name == CGI_MARKER) {
            chosen = None;
        }
        for (name, value) in &self.variables {
            if name.ends_with(PROXY_SUFFIX) && is_kind(name, kind) {
                chosen = (!value.is_empty()).then_some(value.as_slice());
            }
        }
        chosen
    }

    /// `"http" in urllib.request.getproxies_environment()`.
    pub fn has_http_proxy(&self) -> bool {
        self.proxy_value("http").is_some()
    }
}

#[cfg(unix)]
fn os_bytes(text: std::ffi::OsString) -> Vec<u8> {
    std::os::unix::ffi::OsStringExt::into_vec(text)
}

#[cfg(not(unix))]
fn os_bytes(text: std::ffi::OsString) -> Vec<u8> {
    text.to_string_lossy().into_owned().into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(pairs: &[(&str, &str)]) -> ProxyEnvironment {
        ProxyEnvironment::from_pairs(
            pairs
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string())),
        )
    }

    #[test]
    fn http_proxy_variables_follow_getproxies_environment() {
        assert!(!environment(&[]).has_http_proxy());
        assert!(environment(&[("http_proxy", "http://p:1")]).has_http_proxy());
        assert!(environment(&[("HTTP_PROXY", "http://p:1")]).has_http_proxy());
        assert!(!environment(&[("http_proxy", "")]).has_http_proxy());
        assert!(
            !environment(&[("https_proxy", "http://p:1"), ("all_proxy", "x")]).has_http_proxy()
        );
    }

    #[test]
    fn cgi_ignores_the_upper_case_variable_only() {
        assert!(
            !environment(&[("REQUEST_METHOD", "GET"), ("HTTP_PROXY", "http://p:1")])
                .has_http_proxy()
        );
        assert!(
            environment(&[("REQUEST_METHOD", "GET"), ("http_proxy", "http://p:1")])
                .has_http_proxy()
        );
    }

    fn raw(pairs: &[(&[u8], &[u8])]) -> ProxyEnvironment {
        ProxyEnvironment::from_bytes(
            pairs
                .iter()
                .map(|(name, value)| (name.to_vec(), value.to_vec())),
        )
    }

    #[test]
    fn unrelated_non_unicode_entries_never_matter() {
        let unrelated = raw(&[
            (b"VAL_AGENT_PUBLIC_HTTP_FIXTURE", &[0xff]),
            (b"BAD\xffNAME", b"x"),
            (b"BAD\xff_proxy_not", &[0xfe]),
            (b"http_proxy", b""),
        ]);
        assert!(!unrelated.has_http_proxy());
        // the same entries beside a real proxy variable do not hide it
        let mixed = raw(&[(b"X\xff", &[0xff]), (b"http_proxy", b"http://p:1")]);
        assert!(mixed.has_http_proxy());
    }

    #[test]
    fn an_http_proxy_variable_with_non_unicode_bytes_counts_as_set() {
        assert!(raw(&[(b"http_proxy", &[0xff])]).has_http_proxy());
        assert!(raw(&[(b"HTTP_PROXY", &[0xff, 0xfe])]).has_http_proxy());
        // a kind that is not exactly "http" (after lower-casing) is another scheme's proxy
        assert!(!raw(&[(b"http\xff_proxy", b"x")]).has_http_proxy());
        assert!(!raw(&[(b"_proxy", b"x")]).has_http_proxy());
    }

    #[test]
    fn an_empty_lower_case_variable_clears_the_upper_case_one() {
        assert!(!environment(&[("HTTP_PROXY", "http://p:1"), ("http_proxy", "")]).has_http_proxy());
    }
}
