"""Report transport exports; computation and metadata belong to Rust."""
import reporting_native as _c
from reporting_native import (LiteIdentityError, MEASUREMENT_CONFIG_TITLE, MEASUREMENT_CONFIG_KEYS,
    LITE_BENCH_TOOL, fnum, build_md, manifest_path_labels, lite_identity, lite_measurement_config,
    build_lite_md, measurement_config_section, full_measurement_config)
def load(path, label):
    return _c._call("render_report", "load", path=path, label=label)
def repo_root(start):
    return _c._call("render_report", "repo_root", start=start)
def na(v, suffix=_c._MISSING):
    return _c._call("render_report", "na", v=v, **_c._given(suffix=suffix))
