//! hint_api bridge for `render_bench_section` (scripts/render_bench_section.py).
use crate::api::Call;
use crate::core::HintError;
use crate::render_bench_section as r;
use crate::value::J;

pub fn call(k: &Call) -> Option<crate::api::Res> {
    Some(match k.func.as_str() {
        "_num" => r::_num(&k.str(0, "text")),
        "_fmt" => r::_fmt(&k.j(0, "value"), k.int_or(1, "digits", 2)).map(J::Str),
        "parse_report" => r::parse_report(&k.str(0, "text")),
        "is_lite_report" => r::is_lite_report(&k.str(0, "text")).map(J::Bool),
        "lite_metrics_present" => r::lite_metrics_present(&k.str(0, "text")).map(J::Bool),
        "_unescape_cell" => r::_unescape_cell(&k.str(0, "cell")).map(J::Str),
        "_split_row" => r::_split_row(&k.str(0, "line")).map(J::from),
        "parse_measurement_config" => r::parse_measurement_config(&k.str(0, "text")).map(J::from),
        "parse_lite_report" => r::parse_lite_report(&k.str(0, "text")),
        "render_lite" => r::render_lite(&k.j(0, "parsed"), &k.str(1, "source_name")).map(J::Str),
        "parse_sweep_index" => r::parse_sweep_index(&k.j(0, "doc")),
        "render" => r::render(&k.j(0, "parsed"), &k.str(1, "source_name")).map(J::Str),
        "extract_section" => {
            let title = k.opt_str(1, "title").unwrap_or_else(|| r::SECTION_TITLE.to_string());
            r::extract_section(&k.str(0, "text"), &title).map(J::from)
        }
        "_selftest" => Ok(J::Int(r::_selftest() as i64)),
        "main" => {
            let argv = k.strs(0, "argv");
            match r::main_inner(&argv) {
                Err(code) => Err(HintError::py("SystemExit", code.to_string())),
                Ok(res) => res.map(|c| J::Int(c as i64)),
            }
        }
        _ => return None,
    })
}
