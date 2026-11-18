//! One Rust source file → the interpreter launches it contains.
//!
//! syn parses the file. A visitor follows the places where a program is named and works out the static texts each
//! can take: literals (plain, raw, byte, C), `const`/`static`/`let` bindings, path and string constructors
//! (`Path::new`, `PathBuf::from`, `String::from`, `.to_string()`, …), `.join(…)`, `format!`/`concat!`, and the
//! branches of `if`/`match`. The launch sites are
//! - `Command::new(P)` with any path in front (`std::process::`, `tokio::process::`) or a `use … as` alias, together
//!   with the `.arg`/`.args`/`.arg0` calls of its chain or of the variable it is bound to: the program is the
//!   interpreter, `arg0` names it, or a shell (`-c`), wrapper (`env`, `timeout`, `strace`, …) or `ssh` starts it;
//! - argument-vector arrays (`[…]`, `vec![…]`) handed to launch helpers (`run(&["python3", "-m", "venv", dir])`);
//! - calls of a helper of the same file whose parameter becomes the program of `Command::new`.
//!
//! Macro bodies are read as expression lists when they parse as one, and token by token for `Command::new("…")`
//! otherwise (`macro_rules!` rules, `json!` bodies). Text that is only data — a list of forbidden words, a fixture
//! script in a string, a constant nobody launches — is not a launch.

use std::collections::{HashMap, HashSet};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, Lit, Macro, Pat, Stmt, Token};

use super::names::{self, Via, HOLE};
use super::tokens::compact;

/// How deep a value may follow bindings and wrappers (also the guard against `const A = B; const B = A;`).
const MAX_DEPTH: u8 = 12;

/// Zero-argument methods that keep the text of their receiver (`"python3".to_string()`, `py.as_ref()`).
const TEXT_KEEPING_METHODS: [&str; 22] = [
    "to_string",
    "to_owned",
    "into",
    "as_str",
    "as_ref",
    "as_os_str",
    "as_path",
    "clone",
    "to_os_string",
    "into_os_string",
    "to_path_buf",
    "into_boxed_str",
    "to_string_lossy",
    "into_owned",
    "as_deref",
    "borrow",
    "deref",
    "display",
    "as_slice",
    "to_vec",
    "iter",
    "into_iter",
];

/// One-argument constructors that keep the text of their argument (`Path::new("…")`, `OsString::from("…")`).
const TEXT_KEEPING_CONSTRUCTORS: [&str; 9] = [
    "new",
    "from",
    "from_bytes",
    "from_str",
    "from_utf8",
    "from_utf8_lossy",
    "Some",
    "Borrowed",
    "Owned",
];

/// The kind of launch, as printed in a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `Command::new(P)` where P is the interpreter.
    Program,
    /// `.arg0(N)` where N names the interpreter.
    Arg0,
    /// `Command::new(<shell>)` whose `-c` command line starts it.
    Shell,
    /// `Command::new(<env|timeout|strace|…>)` whose arguments start it.
    Wrapper,
    /// `Command::new("ssh")` whose remote command line starts it.
    Remote,
    /// An argument-vector array that starts it.
    Argv,
    /// A call of a same-file helper whose parameter is the program of `Command::new`.
    Helper,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Program => "program",
            Kind::Arg0 => "arg0",
            Kind::Shell => "shell",
            Kind::Wrapper => "wrapper",
            Kind::Remote => "remote",
            Kind::Argv => "argv",
            Kind::Helper => "helper",
        }
    }

    fn of_command(via: Via) -> Kind {
        match via {
            Via::Interpreter => Kind::Program,
            Via::Shell => Kind::Shell,
            Via::Wrapper => Kind::Wrapper,
            Via::Remote => Kind::Remote,
        }
    }
}

/// One interpreter launch in a source file.
#[derive(Clone, Debug)]
pub struct Finding {
    pub line: usize,
    pub kind: Kind,
    /// The normalized call text (`tokens::compact`): with the path, the key of `rs_python_allowlist.tsv`.
    pub call: String,
}

/// How a source file was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    /// As a Rust file (syn): every launch site above.
    Parsed,
    /// Not a Rust file syn parses (a fragment, a fixture): token by token, for `Command::new("…")` only.
    Tokens,
}

/// The launches of one source file, in line order, and how it was read. A source that is not even Rust tokens
/// (an unbalanced delimiter, an unterminated literal) is an error.
pub fn find_launches(source: &str) -> Result<(Vec<Finding>, Reading), String> {
    match syn::parse_file(source) {
        Ok(file) => Ok((find_in_file(&file), Reading::Parsed)),
        Err(_) => {
            let tokens: TokenStream = source.parse().map_err(|e: proc_macro2::LexError| {
                format!("line {}: {e}", e.span().start().line)
            })?;
            let mut finder = Finder::new(Prelude {
                command_names: CommandAliases::default().names,
                ..Prelude::default()
            });
            finder.scan_tokens(tokens);
            Ok((finder.finish(), Reading::Tokens))
        }
    }
}

fn find_in_file(file: &syn::File) -> Vec<Finding> {
    let mut aliases = CommandAliases::default();
    aliases.visit_file(file);
    let mut prelude = Prelude {
        command_names: aliases.names,
        ..Prelude::default()
    };
    let mut collector = PreludeCollector {
        prelude: &mut prelude,
        frames: Vec::new(),
    };
    collector.visit_file(file);
    let mut finder = Finder::new(prelude);
    finder.visit_file(file);
    finder.finish()
}

fn address<T>(node: &T) -> usize {
    node as *const T as usize
}

fn line_of(node: &impl Spanned) -> usize {
    node.span().start().line
}

fn lit_text(lit: &Lit) -> Option<String> {
    match lit {
        Lit::Str(text) => Some(text.value()),
        Lit::ByteStr(bytes) => String::from_utf8(bytes.value()).ok(),
        Lit::CStr(text) => text.value().into_string().ok(),
        _ => None,
    }
}

fn last_segment(path: &syn::Path) -> Option<String> {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// `format!` text with each interpolation as [`HOLE`] (`{{`/`}}` are braces).
fn format_template(format: &str) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
                out.push(HOLE);
            }
            c => out.push(c),
        }
    }
    out
}

/// The identifier a simple binding pattern names (`x`, `mut x`, `x: T`).
fn binding_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) if ident.subpat.is_none() => Some(ident.ident.to_string()),
        Pat::Type(typed) => binding_name(&typed.pat),
        _ => None,
    }
}

/// Every identifier a pattern binds.
fn pattern_names(pat: &Pat, out: &mut Vec<String>) {
    match pat {
        Pat::Ident(ident) => {
            out.push(ident.ident.to_string());
            if let Some((_, sub)) = &ident.subpat {
                pattern_names(sub, out);
            }
        }
        Pat::Type(typed) => pattern_names(&typed.pat, out),
        Pat::Reference(reference) => pattern_names(&reference.pat, out),
        Pat::Paren(paren) => pattern_names(&paren.pat, out),
        Pat::Tuple(tuple) => tuple.elems.iter().for_each(|p| pattern_names(p, out)),
        Pat::TupleStruct(tuple) => tuple.elems.iter().for_each(|p| pattern_names(p, out)),
        Pat::Slice(slice) => slice.elems.iter().for_each(|p| pattern_names(p, out)),
        Pat::Or(or) => or.cases.iter().for_each(|p| pattern_names(p, out)),
        Pat::Struct(fields) => fields
            .fields
            .iter()
            .for_each(|field| pattern_names(&field.pat, out)),
        _ => {}
    }
}

/// The receiver chain of a method call, outermost first, and the expression at its root.
fn method_chain(outer: &ExprMethodCall) -> (Vec<&ExprMethodCall>, &Expr) {
    let mut chain = vec![outer];
    let mut current = outer;
    loop {
        match strip(&current.receiver) {
            Expr::MethodCall(inner) => {
                chain.push(inner);
                current = inner;
            }
            root => return (chain, root),
        }
    }
}

/// The expression under references, parentheses and invisible groups.
fn strip(expr: &Expr) -> &Expr {
    match expr {
        Expr::Reference(reference) => strip(&reference.expr),
        Expr::Paren(paren) => strip(&paren.expr),
        Expr::Group(group) => strip(&group.expr),
        other => other,
    }
}

/// The single identifier an expression names (`cmd`, `&mut cmd`), if it is one.
fn plain_ident(expr: &Expr) -> Option<String> {
    match strip(expr) {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            path.path.get_ident().map(|ident| ident.to_string())
        }
        _ => None,
    }
}

/// The parameter identifier behind a program expression (`program`, `&program`, `program.as_ref()`).
fn parameter_behind(expr: &Expr) -> Option<String> {
    match strip(expr) {
        Expr::MethodCall(call)
            if call.args.is_empty()
                && TEXT_KEEPING_METHODS.contains(&call.method.to_string().as_str()) =>
        {
            parameter_behind(&call.receiver)
        }
        other => plain_ident(other),
    }
}

fn parse_expression_list(mac: &Macro) -> syn::Result<Punctuated<Expr, Token![,]>> {
    mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
}

// ------------------------------------------------------------------------------------------------- prelude passes

/// Names `Command` goes by in this file: `Command` and every `use … Command as X`.
struct CommandAliases {
    names: HashSet<String>,
}

impl Default for CommandAliases {
    fn default() -> Self {
        CommandAliases {
            names: HashSet::from(["Command".to_string()]),
        }
    }
}

impl<'ast> Visit<'ast> for CommandAliases {
    fn visit_use_rename(&mut self, rename: &'ast syn::UseRename) {
        if rename.ident == "Command" {
            self.names.insert(rename.rename.to_string());
        }
    }
}

/// A helper whose parameter at `index` (of `arity`) becomes the program of `Command::new`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HelperParameter {
    index: usize,
    arity: usize,
}

/// What the main pass needs before it starts: names of `Command`, constants, launch helpers.
#[derive(Default)]
struct Prelude {
    command_names: HashSet<String>,
    constants: HashMap<String, Vec<Expr>>,
    helpers: HashMap<String, Vec<HelperParameter>>,
}

impl Prelude {
    /// `Command::new(…)` under any of its names, with any path in front.
    fn command_new<'a>(&self, expr: &'a Expr) -> Option<&'a ExprCall> {
        let Expr::Call(call) = expr else { return None };
        let Expr::Path(func) = &*call.func else {
            return None;
        };
        let segments: Vec<String> = func
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let [.., type_name, constructor] = segments.as_slice() else {
            return None;
        };
        (constructor == "new" && self.command_names.contains(type_name) && call.args.len() == 1)
            .then_some(call)
    }
}

/// Collects constants and launch helpers (functions and `let` closures whose parameter is a `Command::new` program).
struct PreludeCollector<'p> {
    prelude: &'p mut Prelude,
    /// The functions being read, innermost last: name and parameter names (`None` for a pattern that is no name).
    frames: Vec<(String, Vec<Option<String>>)>,
}

impl PreludeCollector<'_> {
    fn in_frame(
        &mut self,
        name: String,
        params: Vec<Option<String>>,
        walk: impl FnOnce(&mut Self),
    ) {
        self.frames.push((name, params));
        walk(self);
        self.frames.pop();
    }

    fn add_constant(&mut self, name: &syn::Ident, expr: &Expr) {
        self.prelude
            .constants
            .entry(name.to_string())
            .or_default()
            .push(expr.clone());
    }
}

/// Every name the typed parameters of a signature bind (they hide outer bindings of the same name).
fn signature_names(signature: &syn::Signature) -> Vec<String> {
    let mut names = Vec::new();
    for input in &signature.inputs {
        if let syn::FnArg::Typed(typed) = input {
            pattern_names(&typed.pat, &mut names);
        }
    }
    names
}

/// The typed parameters of a signature, by position: the name of each simple one.
fn signature_params(signature: &syn::Signature) -> Vec<Option<String>> {
    signature
        .inputs
        .iter()
        .filter_map(|input| match input {
            syn::FnArg::Typed(typed) => Some(binding_name(&typed.pat)),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

impl<'ast> Visit<'ast> for PreludeCollector<'_> {
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.add_constant(&item.ident, &item.expr);
        visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.add_constant(&item.ident, &item.expr);
        visit::visit_item_static(self, item);
    }

    fn visit_impl_item_const(&mut self, item: &'ast syn::ImplItemConst) {
        self.add_constant(&item.ident, &item.expr);
        visit::visit_impl_item_const(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let params = signature_params(&item.sig);
        self.in_frame(item.sig.ident.to_string(), params, |this| {
            visit::visit_item_fn(this, item)
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let params = signature_params(&item.sig);
        self.in_frame(item.sig.ident.to_string(), params, |this| {
            visit::visit_impl_item_fn(this, item)
        });
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let closure = local
            .init
            .as_ref()
            .and_then(|init| match strip(&init.expr) {
                Expr::Closure(closure) => Some(closure),
                _ => None,
            });
        match (binding_name(&local.pat), closure) {
            (Some(name), Some(closure)) => {
                let params = closure.inputs.iter().map(binding_name).collect();
                self.in_frame(name, params, |this| visit::visit_local(this, local));
            }
            _ => visit::visit_local(self, local),
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let (Some(call), Some((name, params))) =
            (self.prelude.command_new(expr), self.frames.last())
        {
            if let Some(param) = parameter_behind(&call.args[0]) {
                if let Some(index) = params
                    .iter()
                    .position(|p| p.as_deref() == Some(param.as_str()))
                {
                    let helper = HelperParameter {
                        index,
                        arity: params.len(),
                    };
                    let known = self.prelude.helpers.entry(name.clone()).or_default();
                    if !known.contains(&helper) {
                        known.push(helper);
                    }
                }
            }
        }
        visit::visit_expr(self, expr);
    }
}

// ---------------------------------------------------------------------------------------------------- values

/// The static texts an expression can take, and its elements when it is a list.
#[derive(Clone, Debug, Default)]
struct Value {
    texts: Vec<String>,
    items: Option<Vec<Vec<String>>>,
}

impl Value {
    fn text(text: Option<String>) -> Value {
        Value {
            texts: text.into_iter().collect(),
            items: None,
        }
    }

    fn texts_of(other: Value) -> Value {
        Value {
            texts: other.texts,
            items: None,
        }
    }

    fn merge(&mut self, other: Value) {
        self.texts.extend(other.texts);
        if self.items.is_none() {
            self.items = other.items;
        }
    }
}

enum Binding {
    Known(Value),
    /// A parameter or pattern binding: its value is not known here (and it hides outer names).
    Opaque,
}

#[derive(Default)]
struct Scope {
    names: HashMap<String, Binding>,
    /// Variables bound to a `Command` (index into `Finder::launches`).
    commands: HashMap<String, usize>,
}

/// A `Command::new` site with what its chain or variable added.
struct Launch {
    line: usize,
    call: String,
    program: Vec<String>,
    args: Vec<Vec<String>>,
    /// `.arg0(…)` calls: line, call text, texts.
    arg0: Vec<(usize, String, Vec<String>)>,
}

struct Finder {
    prelude: Prelude,
    scopes: Vec<Scope>,
    launches: Vec<Launch>,
    /// `Command::new` call node → launch.
    launch_at: HashMap<usize, usize>,
    /// Nodes already read as part of a launch (method calls of a chain, the arrays and `vec!` given to `.args`).
    claimed: HashSet<usize>,
    findings: Vec<Finding>,
    /// Expression lists parsed out of macro bodies, kept until the end so that node addresses stay unique.
    macro_bodies: Vec<Punctuated<Expr, Token![,]>>,
}

impl Finder {
    fn new(prelude: Prelude) -> Self {
        Finder {
            prelude,
            scopes: vec![Scope::default()],
            launches: Vec::new(),
            launch_at: HashMap::new(),
            claimed: HashSet::new(),
            findings: Vec::new(),
            macro_bodies: Vec::new(),
        }
    }

    fn scoped(&mut self, names: Vec<String>, walk: impl FnOnce(&mut Self)) {
        let mut scope = Scope::default();
        for name in names {
            scope.names.insert(name, Binding::Opaque);
        }
        self.scopes.push(scope);
        walk(self);
        self.scopes.pop();
    }

    fn current_scope(&mut self) -> &mut Scope {
        self.scopes
            .last_mut()
            .expect("the file scope is never popped")
    }

    // ---- values

    fn value(&self, expr: &Expr) -> Value {
        self.value_at(expr, 0)
    }

    fn value_at(&self, expr: &Expr, depth: u8) -> Value {
        if depth > MAX_DEPTH {
            return Value::default();
        }
        let next = depth + 1;
        match expr {
            Expr::Lit(lit) => Value::text(lit_text(&lit.lit)),
            Expr::Reference(inner) => self.value_at(&inner.expr, next),
            Expr::Paren(inner) => self.value_at(&inner.expr, next),
            Expr::Group(inner) => self.value_at(&inner.expr, next),
            Expr::Cast(inner) => self.value_at(&inner.expr, next),
            Expr::Try(inner) => self.value_at(&inner.expr, next),
            Expr::Path(path) if path.qself.is_none() => self.path_value(&path.path, next),
            Expr::Array(array) => Value {
                texts: Vec::new(),
                items: Some(
                    array
                        .elems
                        .iter()
                        .map(|e| self.value_at(e, next).texts)
                        .collect(),
                ),
            },
            Expr::Macro(mac) => self.macro_value(&mac.mac, next),
            Expr::MethodCall(call) => self.method_value(call, next),
            Expr::Call(call) => self.call_value(call, next),
            Expr::If(branch) => {
                let mut value = self.block_value(&branch.then_branch, next);
                if let Some((_, otherwise)) = &branch.else_branch {
                    value.merge(self.value_at(otherwise, next));
                }
                value
            }
            Expr::Block(block) => self.block_value(&block.block, next),
            Expr::Match(choice) => choice.arms.iter().fold(Value::default(), |mut value, arm| {
                value.merge(self.value_at(&arm.body, next));
                value
            }),
            _ => Value::default(),
        }
    }

    fn path_value(&self, path: &syn::Path, depth: u8) -> Value {
        let Some(name) = last_segment(path) else {
            return Value::default();
        };
        if path.segments.len() == 1 {
            for scope in self.scopes.iter().rev() {
                match scope.names.get(&name) {
                    Some(Binding::Known(value)) => return value.clone(),
                    Some(Binding::Opaque) => return Value::default(),
                    None => {}
                }
            }
        }
        let mut value = Value::default();
        for expr in self.prelude.constants.get(&name).into_iter().flatten() {
            value.merge(self.value_at(expr, depth));
        }
        value
    }

    fn block_value(&self, block: &syn::Block, depth: u8) -> Value {
        match block.stmts.last() {
            Some(Stmt::Expr(expr, None)) => self.value_at(expr, depth),
            _ => Value::default(),
        }
    }

    fn method_value(&self, call: &ExprMethodCall, depth: u8) -> Value {
        let method = call.method.to_string();
        match (method.as_str(), call.args.len()) {
            (name, 0) if TEXT_KEEPING_METHODS.contains(&name) => {
                self.value_at(&call.receiver, depth)
            }
            // the last component decides the program name: `venv.join("bin/python")`
            ("join" | "with_file_name", 1) => Value::texts_of(self.value_at(&call.args[0], depth)),
            ("unwrap_or", 1) => {
                let mut value = self.value_at(&call.receiver, depth);
                value.merge(self.value_at(&call.args[0], depth));
                value
            }
            ("unwrap_or_else", 1) => match strip(&call.args[0]) {
                Expr::Closure(closure) => self.value_at(&closure.body, depth),
                _ => Value::default(),
            },
            _ => Value::default(),
        }
    }

    fn call_value(&self, call: &ExprCall, depth: u8) -> Value {
        let Expr::Path(func) = &*call.func else {
            return Value::default();
        };
        if call.args.len() != 1 {
            return Value::default();
        }
        let argument = self.value_at(&call.args[0], depth);
        match last_segment(&func.path) {
            Some(name) if TEXT_KEEPING_CONSTRUCTORS.contains(&name.as_str()) => argument,
            // a one-argument helper keeps an argument vector it is given (`strv(&["python3", "-m", …])`)
            _ => Value {
                texts: Vec::new(),
                items: argument.items,
            },
        }
    }

    fn macro_value(&self, mac: &Macro, depth: u8) -> Value {
        let Some(name) = last_segment(&mac.path) else {
            return Value::default();
        };
        let Ok(list) = parse_expression_list(mac) else {
            return Value::default();
        };
        match name.as_str() {
            "vec" => Value {
                texts: Vec::new(),
                items: Some(list.iter().map(|e| self.value_at(e, depth).texts).collect()),
            },
            "format" | "format_args" => match list.first() {
                Some(Expr::Lit(lit)) => {
                    Value::text(lit_text(&lit.lit).map(|format| format_template(&format)))
                }
                _ => Value::default(),
            },
            "concat" => {
                let joined = list
                    .iter()
                    .map(|e| {
                        self.value_at(e, depth)
                            .texts
                            .into_iter()
                            .next()
                            .unwrap_or_else(|| HOLE.to_string())
                    })
                    .collect::<String>();
                Value::text(Some(joined))
            }
            _ => Value::default(),
        }
    }

    // ---- launches

    fn open_launch(&mut self, call: &ExprCall) -> usize {
        if let Some(&index) = self.launch_at.get(&address(call)) {
            return index;
        }
        let program = self.value(&call.args[0]).texts;
        let index = self.launches.len();
        self.launches.push(Launch {
            line: line_of(call),
            call: compact(call.to_token_stream()),
            program,
            args: Vec::new(),
            arg0: Vec::new(),
        });
        self.launch_at.insert(address(call), index);
        index
    }

    /// Adds the `.arg`/`.args`/`.arg0` calls of a chain (innermost first) to a launch.
    fn extend_launch(&mut self, index: usize, methods: &[&ExprMethodCall]) {
        for call in methods.iter().rev() {
            if call.args.len() != 1 {
                continue;
            }
            let argument = &call.args[0];
            match call.method.to_string().as_str() {
                "arg" => {
                    let texts = self.value(argument).texts;
                    self.launches[index].args.push(texts);
                }
                "args" => {
                    self.claim_argument_vector(argument);
                    let items = self
                        .value(argument)
                        .items
                        .unwrap_or_else(|| vec![Vec::new()]);
                    self.launches[index].args.extend(items);
                }
                "arg0" => {
                    let texts = self.value(argument).texts;
                    let text = format!("arg0({})", compact(argument.to_token_stream()));
                    self.launches[index]
                        .arg0
                        .push((line_of(*call), text, texts));
                }
                _ => {}
            }
        }
    }

    /// An array or `vec!` given to `.args` is read with its launch, not again as an argument vector of its own.
    fn claim_argument_vector(&mut self, argument: &Expr) {
        match strip(argument) {
            Expr::Array(array) => {
                self.claimed.insert(address(array));
            }
            Expr::Macro(mac) => {
                self.claimed.insert(address(&mac.mac));
            }
            _ => {}
        }
    }

    fn command_variable(&self, name: &str) -> Option<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&index) = scope.commands.get(name) {
                return Some(index);
            }
            if scope.names.contains_key(name) {
                return None;
            }
        }
        None
    }

    fn check_argument_vector(&mut self, items: &[Vec<String>], line: usize, call: String) {
        if names::argv_starts_interpreter(items).is_some() {
            self.findings.push(Finding {
                line,
                kind: Kind::Argv,
                call,
            });
        }
    }

    fn check_helper_call(&mut self, call: &ExprCall) {
        let Expr::Path(func) = &*call.func else {
            return;
        };
        let Some(name) = last_segment(&func.path) else {
            return;
        };
        let Some(parameters) = self.prelude.helpers.get(&name) else {
            return;
        };
        let starts = parameters.iter().any(|parameter| {
            parameter.arity == call.args.len()
                && call
                    .args
                    .iter()
                    .nth(parameter.index)
                    .is_some_and(|argument| {
                        self.value(argument)
                            .texts
                            .iter()
                            .any(|text| names::is_interpreter(text))
                    })
        });
        if starts {
            self.findings.push(Finding {
                line: line_of(call),
                kind: Kind::Helper,
                call: compact(call.to_token_stream()),
            });
        }
    }

    /// Token-level reading of a macro body that is no expression list: `Command::new("…")` with a literal program.
    fn scan_tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        for (at, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Group(group) => self.scan_tokens(group.stream()),
                TokenTree::Ident(constructor) if constructor == "new" && at >= 3 => {
                    let (
                        TokenTree::Ident(type_name),
                        TokenTree::Punct(first),
                        TokenTree::Punct(second),
                    ) = (&tokens[at - 3], &tokens[at - 2], &tokens[at - 1])
                    else {
                        continue;
                    };
                    let Some(TokenTree::Group(arguments)) = tokens.get(at + 1) else {
                        continue;
                    };
                    if first.as_char() != ':'
                        || second.as_char() != ':'
                        || arguments.delimiter() != Delimiter::Parenthesis
                    {
                        continue;
                    }
                    if !self.prelude.command_names.contains(&type_name.to_string()) {
                        continue;
                    }
                    let program = arguments
                        .stream()
                        .into_iter()
                        .find_map(|inner| match inner {
                            TokenTree::Literal(literal) => lit_text(&Lit::new(literal)),
                            _ => None,
                        });
                    if program.as_deref().is_some_and(names::is_interpreter) {
                        let call: TokenStream = tokens[at - 3..=at + 1].iter().cloned().collect();
                        self.findings.push(Finding {
                            line: type_name.span().start().line,
                            kind: Kind::Program,
                            call: compact(call),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn finish(mut self) -> Vec<Finding> {
        for launch in &self.launches {
            if let Some(via) = names::command_starts_interpreter(&launch.program, &launch.args) {
                self.findings.push(Finding {
                    line: launch.line,
                    kind: Kind::of_command(via),
                    call: launch.call.clone(),
                });
            } else if let Some((line, call, _)) = launch
                .arg0
                .iter()
                .find(|(_, _, texts)| texts.iter().any(|text| names::is_interpreter(text)))
            {
                self.findings.push(Finding {
                    line: *line,
                    kind: Kind::Arg0,
                    call: call.clone(),
                });
            }
        }
        self.findings
            .sort_by(|a, b| (a.line, &a.call).cmp(&(b.line, &b.call)));
        self.findings
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let params = signature_names(&item.sig);
        self.scoped(params, |this| visit::visit_item_fn(this, item));
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let params = signature_names(&item.sig);
        self.scoped(params, |this| visit::visit_impl_item_fn(this, item));
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        let params = signature_names(&item.sig);
        self.scoped(params, |this| visit::visit_trait_item_fn(this, item));
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let mut params = Vec::new();
        closure
            .inputs
            .iter()
            .for_each(|pat| pattern_names(pat, &mut params));
        self.scoped(params, |this| visit::visit_expr_closure(this, closure));
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scoped(Vec::new(), |this| visit::visit_block(this, block));
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        let mut names = Vec::new();
        pattern_names(&arm.pat, &mut names);
        self.scoped(names, |this| visit::visit_arm(this, arm));
    }

    fn visit_expr_for_loop(&mut self, for_loop: &'ast syn::ExprForLoop) {
        self.visit_expr(&for_loop.expr);
        let mut names = Vec::new();
        pattern_names(&for_loop.pat, &mut names);
        self.scoped(names, |this| this.visit_block(&for_loop.body));
    }

    fn visit_expr_let(&mut self, condition: &'ast syn::ExprLet) {
        visit::visit_expr_let(self, condition);
        let mut names = Vec::new();
        pattern_names(&condition.pat, &mut names);
        for name in names {
            self.current_scope().names.insert(name, Binding::Opaque);
        }
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        visit::visit_local(self, local);
        match (binding_name(&local.pat), &local.init) {
            (Some(name), Some(init)) => {
                let value = self.value(&init.expr);
                let launch = match strip(&init.expr) {
                    Expr::MethodCall(call) => self.prelude.command_new(method_chain(call).1),
                    other => self.prelude.command_new(other),
                }
                .and_then(|call| self.launch_at.get(&address(call)).copied());
                let scope = self.current_scope();
                scope.names.insert(name.clone(), Binding::Known(value));
                match launch {
                    Some(index) => scope.commands.insert(name, index),
                    None => scope.commands.remove(&name),
                };
            }
            _ => {
                let mut names = Vec::new();
                pattern_names(&local.pat, &mut names);
                let scope = self.current_scope();
                for name in names {
                    scope.commands.remove(&name);
                    scope.names.insert(name, Binding::Opaque);
                }
            }
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::Call(call) => {
                if self.prelude.command_new(expr).is_some() {
                    self.open_launch(call);
                }
                self.check_helper_call(call);
            }
            Expr::MethodCall(outer) if !self.claimed.contains(&address(outer)) => {
                let (chain, root) = method_chain(outer);
                for link in &chain {
                    self.claimed.insert(address(*link));
                }
                if let Some(call) = self.prelude.command_new(root) {
                    let index = self.open_launch(call);
                    self.extend_launch(index, &chain);
                } else if let Some(index) =
                    plain_ident(root).and_then(|name| self.command_variable(&name))
                {
                    self.extend_launch(index, &chain);
                }
            }
            Expr::Array(array) if !self.claimed.contains(&address(array)) => {
                let items: Vec<Vec<String>> =
                    array.elems.iter().map(|e| self.value(e).texts).collect();
                self.check_argument_vector(
                    &items,
                    line_of(array),
                    compact(array.to_token_stream()),
                );
            }
            _ => {}
        }
        visit::visit_expr(self, expr);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        match parse_expression_list(mac) {
            Ok(list) => {
                if last_segment(&mac.path).as_deref() == Some("vec")
                    && !self.claimed.contains(&address(mac))
                {
                    let items: Vec<Vec<String>> =
                        list.iter().map(|e| self.value(e).texts).collect();
                    self.check_argument_vector(
                        &items,
                        line_of(mac),
                        compact(mac.to_token_stream()),
                    );
                }
                for expr in &list {
                    <Self as Visit<'_>>::visit_expr(self, expr);
                }
                self.macro_bodies.push(list);
            }
            Err(_) => self.scan_tokens(mac.tokens.clone()),
        }
    }
}
