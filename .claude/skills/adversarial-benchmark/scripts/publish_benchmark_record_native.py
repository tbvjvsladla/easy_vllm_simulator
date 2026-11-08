"""Certificate transport exports; publication rules belong to Rust."""
import reporting_native as _c
from reporting_native import (ENTRY_BETA, ENTRY_GAMMA, LITE_CERT_SOFT_KEYS,
    scalar, build_yaml, build_lite_yaml)
def load(path, label):
    return _c._call("publish_benchmark_record", "load", path=path, label=label)
def repo_root(start):
    return _c._call("publish_benchmark_record", "repo_root", start=start)
