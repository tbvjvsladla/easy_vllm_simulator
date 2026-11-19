//! Shared support of the model checks: the saved reference data, a scratch copy of the saved input tree, the rebasing of saved
//! strings, and the loopback HTTP fixture the capture used (extended with two stalling answers for the timeout checks).
//! A module of the `model_checks` target, shared by its three check modules.

use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::model_files::PrivateDirectory;
use recipe_runtime::run_trial_session::utc_iso;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// A TCP socket bound to a loopback port chosen by the system and never put into the listening state, owned for as long as
/// the value lives: the port cannot be bound by anyone else meanwhile, and a connection to it is refused (`ECONNREFUSED`).
/// Written against the C library directly (the standard library cannot bind without listening); test-only, Linux GNU.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub struct RefusingPort {
    _socket: std::os::fd::OwnedFd,
    pub port: u16,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod libc_socket {
    #[repr(C)]
    pub struct SockAddrIn {
        pub family: u16,
        pub port: u16,
        pub address: u32,
        pub zero: [u8; 8],
    }

    extern "C" {
        pub fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
        pub fn bind(descriptor: i32, address: *const SockAddrIn, length: u32) -> i32;
        pub fn getsockname(descriptor: i32, address: *mut SockAddrIn, length: *mut u32) -> i32;
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
impl RefusingPort {
    pub fn bind() -> RefusingPort {
        use libc_socket::{bind, getsockname, socket, SockAddrIn};
        use std::os::fd::{FromRawFd, OwnedFd};
        const AF_INET: i32 = 2;
        const SOCK_STREAM: i32 = 1;
        const SOCK_CLOEXEC: i32 = 0o2000000;
        let size = std::mem::size_of::<SockAddrIn>() as u32;
        // SAFETY: plain system calls on a descriptor created here and handed to an `OwnedFd` at once; the address structure
        // is the C `sockaddr_in` of Linux (family, port and address in network order, eight zero bytes).
        unsafe {
            let descriptor = socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC, 0);
            assert!(
                descriptor >= 0,
                "socket: {}",
                std::io::Error::last_os_error()
            );
            let owned = OwnedFd::from_raw_fd(descriptor);
            let wanted = SockAddrIn {
                family: AF_INET as u16,
                port: 0,
                address: u32::from_be_bytes([127, 0, 0, 1]).to_be(),
                zero: [0; 8],
            };
            assert_eq!(
                bind(descriptor, &wanted, size),
                0,
                "bind: {}",
                std::io::Error::last_os_error()
            );
            let mut actual = SockAddrIn {
                family: 0,
                port: 0,
                address: 0,
                zero: [0; 8],
            };
            let mut length = size;
            assert_eq!(
                getsockname(descriptor, &mut actual, &mut length),
                0,
                "getsockname: {}",
                std::io::Error::last_os_error()
            );
            RefusingPort {
                _socket: owned,
                port: u16::from_be(actual.port),
            }
        }
    }

    /// The base URL that names this very socket.
    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// What the fixture saw of one request: the path and the `Accept` header.
pub type Seen = (String, Option<String>);

pub const TOKEN: &str = "@REFERENCE@";
pub const SAVED_BASE: &str = "http://127.0.0.1:30067";

pub fn data(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("checks")
        .join("data")
        .join("model_reference")
        .join(relative)
}

pub fn load(relative: &str) -> PyValue {
    let text = std::fs::read_to_string(data(relative)).expect("saved data");
    loads_text(&PyStr::new(&text)).expect("saved JSON")
}

pub fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("directory");
    for entry in std::fs::read_dir(from).expect("read directory") {
        let entry = entry.expect("entry");
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name()))
                .expect("copy a saved input file");
        }
    }
}

/// A scratch copy of the saved input tree (the directory the original saw as `input/`), created exclusively and removed at the end.
pub struct Scratch {
    _directory: PrivateDirectory,
    pub root: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        let directory = PrivateDirectory::create(&format!("model-reference-{label}-"))
            .expect("a private scratch directory");
        let root = directory.path().to_path_buf();
        copy_tree(&data("input"), &root.join("input"));
        // Git keeps no empty directory: the saved tree has one (`empty`).
        std::fs::create_dir_all(root.join("input").join("empty"))
            .expect("the empty model directory");
        Scratch {
            _directory: directory,
            root,
        }
    }
}

/// Every string of the value with the token replaced (`token_value` for the capture directory, `base` for the HTTP base).
pub fn rebased(value: &PyValue, replacements: &[(&str, String)]) -> PyValue {
    let fix = |text: &PyStr| -> PyStr {
        let mut owned = text.to_utf8().unwrap_or_default();
        if text.to_utf8().is_none() {
            return text.clone();
        }
        for (from, to) in replacements {
            owned = owned.replace(from, to);
        }
        PyStr::new(&owned)
    };
    match value {
        PyValue::Str(text) => PyValue::Str(fix(text)),
        PyValue::List(items) => PyValue::List(
            items
                .iter()
                .map(|item| rebased(item, replacements))
                .collect(),
        ),
        PyValue::Tuple(items) => PyValue::Tuple(
            items
                .iter()
                .map(|item| rebased(item, replacements))
                .collect(),
        ),
        PyValue::Dict(map) => {
            let mut copy = PyDict::new();
            for (key, item) in map.iter() {
                copy.insert(fix(key), rebased(item, replacements));
            }
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

pub fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("missing {key}")),
        other => panic!("not an object: {}", other.repr()),
    }
}

pub fn items(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not a list: {}", other.repr()),
    }
}

pub fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

pub struct Fixture {
    pub base: String,
    requests: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Answers added to the capture's: a path and the body it gets (status 200).
pub type Route = (String, Vec<u8>);

fn canned(path: &str, extra: &[Route]) -> (u16, Vec<u8>) {
    if let Some((_, body)) = extra.iter().find(|(route, _)| route == path) {
        return (200, body.clone());
    }
    let name = path.trim_start_matches('/');
    let status = if name == "forbidden" { 403 } else { 200 };
    let body: Vec<u8> = match name {
        "badjson" => b"{\r\n".to_vec(),
        "badutf8" => b"{\"x\":\"\xff\"}".to_vec(),
        "ok" => {
            br#"{"safetensors": {"total": 100, "parameters": {"BF16": 75, "F32": 25}}}"#.to_vec()
        }
        "zero" => br#"{"safetensors": {"total": 0}}"#.to_vec(),
        "badtotal" => br#"{"safetensors": {"total": "bad"}}"#.to_vec(),
        "array" => b"[]".to_vec(),
        _ => b"{}".to_vec(),
    };
    (status, body)
}

fn serve_stream<S: Read + Write>(mut stream: S, requests: &Mutex<Vec<Seen>>, extra: &[Route]) {
    let mut head: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                head.push(byte[0]);
                if head.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let mut lines = text.lines();
    let path = lines
        .next()
        .and_then(|first| first.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    let accept = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
        .map(|(_, value)| value.trim().to_string());
    requests
        .lock()
        .expect("requests")
        .push((path.clone(), accept));
    if path == "/stall" {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        return;
    }
    if path == "/bodystall" {
        let head =
            "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n";
        if stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(b"{\"a\":"))
            .and_then(|()| stream.flush())
            .is_ok()
        {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        return;
    }
    let (status, body) = canned(&path, extra);
    let reason = if status == 403 { "Forbidden" } else { "OK" };
    let response = format!(
        "HTTP/1.0 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    if stream
        .write_all(response.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .and_then(|()| stream.flush())
        .is_err()
    {
        eprintln!("fixture: a client went away");
    }
}

type Wrap = dyn Fn(TcpStream, &Mutex<Vec<Seen>>, &[Route]) + Send + Sync;

fn run_listener(
    listener: TcpListener,
    scheme: &str,
    extra: Vec<Route>,
    wrap: Arc<Wrap>,
) -> Fixture {
    let port = listener.local_addr().expect("address").port();
    listener.set_nonblocking(true).expect("non-blocking accept");
    let requests: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (shared, flag) = (Arc::clone(&requests), Arc::clone(&stop));
    let extra = Arc::new(extra);
    let thread = std::thread::spawn(move || loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(error) = stream.set_nonblocking(false) {
                    eprintln!("fixture: {error}");
                    continue;
                }
                let (per_connection, routes, wrap) =
                    (Arc::clone(&shared), Arc::clone(&extra), Arc::clone(&wrap));
                std::thread::spawn(move || wrap(stream, &per_connection, &routes));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => {
                eprintln!("fixture accept: {error}");
                break;
            }
        }
    });
    Fixture {
        base: format!("{scheme}://127.0.0.1:{port}"),
        requests,
        stop,
        thread: Some(thread),
    }
}

impl Fixture {
    pub fn start() -> Fixture {
        Fixture::start_with(Vec::new())
    }

    /// The capture's plain HTTP fixture (127.0.0.1:30067 when free, else a free port) with extra answers.
    pub fn start_with(extra: Vec<Route>) -> Fixture {
        let listener = TcpListener::bind("127.0.0.1:30067")
            .or_else(|_| TcpListener::bind("127.0.0.1:0"))
            .expect("a loopback port");
        run_listener(listener, "http", extra, Arc::new(serve_stream))
    }

    /// A plain HTTP fixture on a free loopback port that answers every request with `302 Found` to `location_base` + the request
    /// path (another origin, for instance the TLS fixture), recording what it was asked.
    pub fn start_redirecting(location_base: &str) -> Fixture {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let base = location_base.to_string();
        run_listener(
            listener,
            "http",
            Vec::new(),
            Arc::new(move |mut stream, requests, _routes| {
                let mut head: Vec<u8> = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut byte) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => head.push(byte[0]),
                    }
                }
                let text = String::from_utf8_lossy(&head).into_owned();
                let mut lines = text.lines();
                let path = lines
                    .next()
                    .and_then(|first| first.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let accept = lines
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
                    .map(|(_, value)| value.trim().to_string());
                requests
                    .lock()
                    .expect("requests")
                    .push((path.clone(), accept));
                let response = format!(
                    "HTTP/1.0 302 Found\r\nLocation: {base}{path}\r\nContent-Length: 0\r\n\r\n"
                );
                if stream.write_all(response.as_bytes()).is_err() {
                    eprintln!("fixture: a client went away");
                }
            }),
        )
    }

    pub fn received(&self) -> Vec<Seen> {
        self.requests.lock().expect("requests").clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("fixture thread panicked");
            }
        }
    }
}

/// Runs the `openssl` command line; the program is used only to make fixtures and to answer the hash of a certificate.
pub fn openssl(arguments: &[&str]) -> String {
    let output = std::process::Command::new("openssl")
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap_or_else(|error| {
            panic!("NOT_RUN: the openssl command line is needed to make fixtures: {error}")
        });
    assert!(
        output.status.success(),
        "openssl {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn openssl_days_from_now(days: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after the epoch")
        .as_secs();
    let now = i64::try_from(now).expect("seconds since the epoch fit i64");
    utc_iso(now + days * 86_400).replace(['-', 'T', ':'], "")
}

/// Two certificate authorities (`ca`, `other`) and a server certificate for 127.0.0.1 / localhost signed by `ca`, in a private
/// directory (certificates live two days: they are made for each run, never stored).
pub struct Pki {
    directory: PrivateDirectory,
}

impl Pki {
    pub fn generate() -> Pki {
        let directory = PrivateDirectory::create("model-pki-").expect("a private directory");
        let pki = Pki { directory };
        let path = |name: &str| pki.file(name).to_string_lossy().into_owned();
        for name in ["ca", "other"] {
            openssl(&[
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-keyout",
                &path(&format!("{name}.key")),
                "-out",
                &path(&format!("{name}.pem")),
                "-days",
                "2",
                "-subj",
                &format!("/CN=local-fixture-{name}"),
            ]);
        }
        openssl(&[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            &path("server.key"),
            "-out",
            &path("server.csr"),
            "-subj",
            "/CN=localhost",
        ]);
        std::fs::write(
            pki.file("server.ext"),
            "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\n",
        )
        .expect("write the extension file");
        openssl(&[
            "x509",
            "-req",
            "-in",
            &path("server.csr"),
            "-CA",
            &path("ca.pem"),
            "-CAkey",
            &path("ca.key"),
            "-CAcreateserial",
            "-out",
            &path("server.pem"),
            "-days",
            "2",
            "-extfile",
            &path("server.ext"),
        ]);
        pki
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// A three-level chain made with the command line: `chain-root` (self-signed), `chain-intermediate` (CA, path length 0,
    /// signed by the root) and `chain-leaf` (127.0.0.1 / localhost, signed by the intermediate); each as `<name>.pem`/`.key`.
    pub fn chain(&self) {
        let path = |file: &str| self.file(file).to_string_lossy().into_owned();
        openssl(&[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            &path("chain-root.key"),
            "-out",
            &path("chain-root.pem"),
            "-days",
            "2",
            "-subj",
            "/CN=chain-root",
        ]);
        let issue = |name: &str, parent: &str, subject: &str, extension: &str| {
            openssl(&[
                "req",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-keyout",
                &path(&format!("{name}.key")),
                "-out",
                &path(&format!("{name}.csr")),
                "-subj",
                subject,
            ]);
            std::fs::write(self.file(&format!("{name}.ext")), extension)
                .expect("write the extension file");
            openssl(&[
                "x509",
                "-req",
                "-in",
                &path(&format!("{name}.csr")),
                "-CA",
                &path(&format!("{parent}.pem")),
                "-CAkey",
                &path(&format!("{parent}.key")),
                "-CAcreateserial",
                "-out",
                &path(&format!("{name}.pem")),
                "-days",
                "2",
                "-extfile",
                &path(&format!("{name}.ext")),
            ]);
        };
        issue(
            "chain-intermediate",
            "chain-root",
            "/CN=chain-intermediate",
            "basicConstraints=critical,CA:TRUE,pathlen:0\nkeyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n",
        );
        issue(
            "chain-leaf",
            "chain-intermediate",
            "/CN=localhost",
            "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\nauthorityKeyIdentifier=keyid\n",
        );
    }

    /// A server certificate named `<name>.pem` (key `<name>.key`) signed by `ca`, valid over `validity_days_from_now` and naming
    /// the given subject alternative names, made with `openssl ca` (the only way to back-date a certificate with this command
    /// line).
    pub fn issue(&self, name: &str, validity_days_from_now: Range<i64>, alternative_names: &str) {
        let path = |file: &str| self.file(file).to_string_lossy().into_owned();
        if !self.file("ca.cnf").exists() {
            std::fs::write(
                self.file("ca.cnf"),
                format!(
                    "[ca]\ndefault_ca=d\n[d]\ndatabase={}\nnew_certs_dir={}\nserial={}\ndefault_md=sha256\npolicy=p\nunique_subject=no\ncopy_extensions=copy\n[p]\ncommonName=supplied\n",
                    path("index.txt"),
                    self.directory.path().to_string_lossy(),
                    path("serial.txt")
                ),
            )
            .expect("write the authority configuration");
            std::fs::write(self.file("index.txt"), "").expect("an empty index");
            std::fs::write(self.file("serial.txt"), "10\n").expect("a serial");
        }
        openssl(&[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            &path(&format!("{name}.key")),
            "-out",
            &path(&format!("{name}.csr")),
            "-subj",
            "/CN=localhost",
        ]);
        std::fs::write(
            self.file(&format!("{name}.ext")),
            format!("subjectAltName={alternative_names}\nextendedKeyUsage=serverAuth\n"),
        )
        .expect("write the extension file");
        openssl(&[
            "ca",
            "-batch",
            "-config",
            &path("ca.cnf"),
            "-cert",
            &path("ca.pem"),
            "-keyfile",
            &path("ca.key"),
            "-in",
            &path(&format!("{name}.csr")),
            "-out",
            &path(&format!("{name}.pem")),
            "-startdate",
            &openssl_days_from_now(validity_days_from_now.start),
            "-enddate",
            &openssl_days_from_now(validity_days_from_now.end),
            "-extfile",
            &path(&format!("{name}.ext")),
            "-notext",
        ]);
    }

    /// A self-signed root whose own validity ended two years before the run (`old-root`) and a leaf for 127.0.0.1 / localhost
    /// (`old-leaf`) signed by it that is valid now: the chain is sound except for the dates of its anchor. Made with `openssl ca`
    /// (to back-date).
    pub fn expired_anchor(&self) {
        let path = |file: &str| self.file(file).to_string_lossy().into_owned();
        std::fs::write(
            self.file("old.cnf"),
            format!(
                "[ca]\ndefault_ca=d\n[d]\ndatabase={}\nnew_certs_dir={}\nserial={}\ndefault_md=sha256\npolicy=p\nunique_subject=no\ncopy_extensions=copy\n[p]\ncommonName=supplied\n",
                path("old-index.txt"),
                self.directory.path().to_string_lossy(),
                path("old-serial.txt")
            ),
        )
        .expect("write the authority configuration");
        std::fs::write(self.file("old-index.txt"), "").expect("an empty index");
        std::fs::write(self.file("old-serial.txt"), "20\n").expect("a serial");
        std::fs::write(
            self.file("old-root.ext"),
            "basicConstraints=critical,CA:TRUE\nsubjectKeyIdentifier=hash\n",
        )
        .expect("write the extension file");
        openssl(&[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            &path("old-root.key"),
            "-out",
            &path("old-root.csr"),
            "-subj",
            "/CN=old-root",
        ]);
        openssl(&[
            "ca",
            "-batch",
            "-config",
            &path("old.cnf"),
            "-selfsign",
            "-keyfile",
            &path("old-root.key"),
            "-in",
            &path("old-root.csr"),
            "-out",
            &path("old-root.pem"),
            "-startdate",
            &openssl_days_from_now(-730),
            "-enddate",
            &openssl_days_from_now(-729),
            "-extfile",
            &path("old-root.ext"),
            "-notext",
        ]);
        openssl(&[
            "req",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            &path("old-leaf.key"),
            "-out",
            &path("old-leaf.csr"),
            "-subj",
            "/CN=localhost",
        ]);
        std::fs::write(
            self.file("old-leaf.ext"),
            "subjectAltName=DNS:localhost,IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n",
        )
        .expect("write the extension file");
        openssl(&[
            "ca",
            "-batch",
            "-config",
            &path("old.cnf"),
            "-cert",
            &path("old-root.pem"),
            "-keyfile",
            &path("old-root.key"),
            "-in",
            &path("old-leaf.csr"),
            "-out",
            &path("old-leaf.pem"),
            "-startdate",
            &openssl_days_from_now(-365),
            "-enddate",
            &openssl_days_from_now(365),
            "-extfile",
            &path("old-leaf.ext"),
            "-notext",
        ]);
    }

    /// The text of a certificate file.
    pub fn pem(&self, name: &str) -> String {
        std::fs::read_to_string(self.file(name)).expect("a certificate file")
    }

    /// `openssl x509 -subject_hash` of a certificate file: the oracle of the name a hashed directory must give it.
    pub fn subject_hash(&self, name: &str) -> String {
        openssl(&[
            "x509",
            "-in",
            &self.file(name).to_string_lossy(),
            "-noout",
            "-subject_hash",
        ])
    }
}

fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn pem_block(path: &Path, label: &str) -> Vec<u8> {
    let text = std::fs::read_to_string(path).expect("a PEM file");
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let body = text
        .split_once(&begin)
        .and_then(|(_, rest)| rest.split_once(&end))
        .map(|(body, _)| body)
        .expect("the PEM block");
    let mut out = Vec::new();
    let (mut accumulator, mut bits) = (0u32, 0u32);
    for byte in body
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
    {
        accumulator = (accumulator << 6) | base64_value(byte).expect("base64");
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    out
}

impl Fixture {
    /// The same answers over TLS with the server certificate of `pki` (127.0.0.1 on a free port).
    pub fn start_tls(pki: &Pki, extra: Vec<Route>) -> Fixture {
        Fixture::start_tls_as(pki, "server", extra)
    }

    /// The same with the certificate and key `<name>.pem` / `<name>.key` of `pki`.
    pub fn start_tls_as(pki: &Pki, name: &str, extra: Vec<Route>) -> Fixture {
        Fixture::start_tls_chain(pki, &[name], name, extra)
    }

    /// The same sending the certificates of the files `<sent>.pem` (the first is the end-entity certificate) and signing
    /// the handshake with the key `<key_name>.key` of `pki`.
    pub fn start_tls_chain(pki: &Pki, sent: &[&str], key_name: &str, extra: Vec<Route>) -> Fixture {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let certificates: Vec<CertificateDer<'static>> = sent
            .iter()
            .map(|name| {
                CertificateDer::from(pem_block(&pki.file(&format!("{name}.pem")), "CERTIFICATE"))
            })
            .collect();
        let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(pem_block(
            &pki.file(&format!("{key_name}.key")),
            "PRIVATE KEY",
        )));
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certificates, key)
                .expect("a server configuration"),
        );
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        run_listener(
            listener,
            "https",
            extra,
            Arc::new(move |stream, requests, routes| {
                let connection = match rustls::ServerConnection::new(Arc::clone(&config)) {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("tls fixture: {error}");
                        return;
                    }
                };
                let mut tls = rustls::StreamOwned::new(connection, stream);
                serve_stream(&mut tls, requests, routes);
                tls.conn.send_close_notify();
                let _ = tls.flush();
            }),
        )
    }
}
