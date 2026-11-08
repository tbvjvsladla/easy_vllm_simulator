"""Envelope transport exports; computation belongs to Rust."""
import reporting_native as _c
from reporting_native import max_report_build_md as build_md
def load(path, label):
    return _c._call("render_max_report", "load", path=path, label=label)
def repo_root(start):
    return _c._call("render_max_report", "repo_root", start=start)
def na(v):
    return _c._call("render_max_report", "na", v=v)
