#!/usr/bin/env python3
"""Generic source-mapping generator (metadata authoring only).

Reads the TEXT of the PyYAML source files named on the command line with `ast` (nothing from PyYAML is imported or executed) and prints a Rust
table of every call site: (file, enclosing `Class.function`, callee name, line of the call expression) and every `raise` site (file, enclosing function,
line, exception class). The output is data about the source, not about any input.
"""
import ast, sys, os

def functions(tree):
    """(qualified name, FunctionDef) for every function, `Class.name` inside a class."""
    out = []
    def walk(node, prefix):
        for child in ast.iter_child_nodes(node):
            if isinstance(child, ast.ClassDef):
                walk(child, child.name + ".")
            elif isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                out.append((prefix + child.name, child))
    walk(tree, "")
    return out

def own_nodes(fn):
    # nodes of the function body, not of nested function definitions
    todo = list(fn.body)
    while todo:
        n = todo.pop()
        yield n
        for c in ast.iter_child_nodes(n):
            if isinstance(c, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
                continue
            todo.append(c)

def callee(call):
    f = call.func
    if isinstance(f, ast.Attribute): return f.attr
    if isinstance(f, ast.Name): return f.id
    return None

root, files = sys.argv[1], sys.argv[2:]
calls, raises, defs = [], [], []
for rel in files:
    src = open(os.path.join(root, rel), encoding="utf-8").read()
    tree = ast.parse(src)
    for qual, fn in functions(tree):
        defs.append((rel, qual, fn.lineno))
        for n in own_nodes(fn):
            if isinstance(n, ast.Call):
                c = callee(n)
                if c: calls.append((rel, qual, c, n.lineno))
            elif isinstance(n, ast.Raise):
                cls = None
                if isinstance(n.exc, ast.Call): cls = callee(n.exc)
                elif isinstance(n.exc, ast.Name): cls = n.exc.id
                raises.append((rel, qual, n.lineno, cls or ""))
calls = sorted(set(calls)); raises = sorted(set(raises)); defs = sorted(set(defs))
print("//! GENERATED source map of PyYAML 6.0.1 (the frozen reference's package): every call site and every raise site of the pure-Python modules, read from")
print("//! the source TEXT by `tools/gen_yaml_sites.py` (a generic `ast` walk; no PyYAML code is imported or executed, no input is consulted).")
print("//! Derived from PyYAML 6.0.1, MIT License, Copyright (c) 2017-2021 Ingy döt Net and (c) 2006-2016 Kirill Simonov; see notices/NOTICE.md.\n")
print("/// `(file, enclosing function, callee name, line of the call expression)`, sorted.")
print("pub(crate) const CALLS: &[(&str, &str, &str, u32)] = &[")
for r in calls: print('    ("%s", "%s", "%s", %d),' % r)
print("];\n")
print("/// `(file, enclosing function, line, exception class)` of every `raise`, sorted.")
print("pub(crate) const RAISES: &[(&str, &str, u32, &str)] = &[")
for r in raises: print('    ("%s", "%s", %d, "%s"),' % r)
print("];\n")
print("/// `(file, function, line of its `def`)`, sorted (a name defined in several files appears once per file).")
print("pub(crate) const DEFS: &[(&str, &str, u32)] = &[")
for r in defs: print('    ("%s", "%s", %d),' % r)
print("];")
