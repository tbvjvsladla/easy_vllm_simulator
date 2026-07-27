# hy3_patch.py — Tencent Hy3 (:opensource 특수토큰 접미어) 파서 호환 런타임 패치 (휘발·비추적, per-model).
#
# WHAT : HYV3 reasoning 파서와 tool 파서가 BARE 토큰(<think>·<tool_calls>·<tool_call>·<tool_sep>·<arg_key>·<arg_value>…)을
#        하드코딩하는데, 이 체크포인트(kodelow/Hy3-NVFP4-W4A16)의 vocab 은 :opensource 접미어 형태(<think:opensource>·
#        <tool_calls:opensource>…) '만' 가진다(bare 부재 실측). → vocab.get(bare)=None → 두 파서의 __init__ 이 RuntimeError.
#        이 패치가 토큰 문자열을 :opensource 형으로 스왑(+ id/regex 재계산)해 파서를 정상 구성한다.
# WHY  : 에이전트-레디 서빙(Hermes agent + OpenWebUI 의 tool-call + reasoning 분리). 동일-HW tonyd2wild 2x-DGX-Spark 동일 처방(sed).
# REF  : 로컬 tokenizer_config.json(added_tokens_decoder 의 :opensource 토큰) · vllm 0.24.1
#        vllm/reasoning/hy_v3_reasoning_parser.py(@property start_token/end_token) ·
#        vllm/tool_parsers/hy_v3_tool_parser.py(__init__ 토큰블럭·3 regex·4 token-id·말미 RuntimeError) 소스 직독.
#        (헌법 「모델구동 런타임 패치 따름정리」 — 참조-그라운디드 생성, 환경·bump 마다 재유도·carry-forward ✗.)
# RETIRE-WHEN : 체크포인트가 bare 토큰을 싣거나(KV-보정 리비전) vLLM 파서가 config/tokenizer 에서 토큰을 읽도록 바뀌면 불필요.
# SITE-INIT SAFE : 최상위에서 vllm/transformers 를 import 하지 않는다(site-init 파괴 회피 — 헌법 arm_patch 전제).
#        meta-path finder 로 두 파서 모듈 로드 *직후* 패치 → engine + 모든 Ray/TP worker 프로세스에 적용.
import sys, importlib.util, importlib.abc, re

_SUF = ":opensource"


def _patch_reasoning(mod):
    # BaseThinkingReasoningParser.__init__ 이 self.start_token/end_token(@property)로 id 계산 후 None 이면 RuntimeError.
    # → 서브클래스 property 를 :opensource 형으로 재정의하면 __init__ 이 올바른 id 를 계산(래핑 불요).
    try:
        cls = mod.HYV3ReasoningParser
        cls.start_token = property(lambda self: "<think%s>" % _SUF)
        cls.end_token = property(lambda self: "</think%s>" % _SUF)
        sys.stderr.write("[hy3-patch] reasoning parser tokens -> :opensource\n")
    except Exception as e:
        sys.stderr.write("[hy3-patch] reasoning warn: %s\n" % e)


def _patch_tool(mod):
    # HYV3ToolParser.__init__ 은 bare 토큰 설정 → 3 regex 컴파일 → 4 token-id(vocab.get=None) → 말미 RuntimeError.
    # 원 __init__ 은 super().__init__(vocab/tokenizer 세팅)·streaming-state·regex 를 모두 설정한 *뒤* 마지막에 raise 하므로,
    # try/except 로 그 raise 만 삼키고 토큰 9개 재지정 + regex 3개 재컴파일 + id 4개 재계산으로 마무리한다(구조 동일).
    try:
        cls = mod.HYV3ToolParser
        _orig = cls.__init__

        def _init(self, *a, **kw):
            try:
                _orig(self, *a, **kw)   # bare 토큰/regex/None-id 설정 후 RuntimeError(bare 부재) → 아래서 교정
            except RuntimeError:
                pass
            self.tool_calls_start_token = "<tool_calls%s>" % _SUF
            self.tool_calls_end_token = "</tool_calls%s>" % _SUF
            self.tool_call_start_token = "<tool_call%s>" % _SUF
            self.tool_call_end_token = "</tool_call%s>" % _SUF
            self.tool_sep_token = "<tool_sep%s>" % _SUF
            self.arg_key_start_token = "<arg_key%s>" % _SUF
            self.arg_key_end_token = "</arg_key%s>" % _SUF
            self.arg_value_start_token = "<arg_value%s>" % _SUF
            self.arg_value_end_token = "</arg_value%s>" % _SUF
            # regex 3개 재컴파일(원 소스와 동일 구조 — 토큰에 regex 메타문자 없음).
            self.tool_call_regex = re.compile(
                rf"{self.tool_call_start_token}(.*?){self.tool_sep_token}"
                rf"(.*?){self.tool_call_end_token}",
                re.DOTALL,
            )
            self.tool_call_portion_regex = re.compile(
                rf"{self.tool_call_start_token}(.*?){self.tool_sep_token}(.*)", re.DOTALL
            )
            self.func_args_regex = re.compile(
                rf"{self.arg_key_start_token}(.*?){self.arg_key_end_token}\s*"
                rf"{self.arg_value_start_token}(.*?){self.arg_value_end_token}",
                re.DOTALL,
            )
            # token-id 4개 재계산(super().__init__ 이 self.vocab 세팅 완료).
            self.tool_calls_start_token_id = self.vocab.get(self.tool_calls_start_token)
            self.tool_calls_end_token_id = self.vocab.get(self.tool_calls_end_token)
            self.tool_call_start_token_id = self.vocab.get(self.tool_call_start_token)
            self.tool_call_end_token_id = self.vocab.get(self.tool_call_end_token)
            if not hasattr(self, "_buffer"):
                self._buffer = ""
            if self.tool_calls_start_token_id is None or self.tool_calls_end_token_id is None:
                sys.stderr.write("[hy3-patch] WARN: :opensource tool tokens STILL absent in vocab!\n")
            else:
                sys.stderr.write("[hy3-patch] tool parser tokens -> :opensource (ids ok)\n")

        cls.__init__ = _init
    except Exception as e:
        sys.stderr.write("[hy3-patch] tool warn: %s\n" % e)


_P = {
    "vllm.reasoning.hy_v3_reasoning_parser": _patch_reasoning,
    "vllm.tool_parsers.hy_v3_tool_parser": _patch_tool,
}


class _F(importlib.abc.MetaPathFinder):
    def find_spec(self, name, path, target=None):
        if name not in _P:
            return None
        sys.meta_path.remove(self)
        try:
            spec = importlib.util.find_spec(name)
        finally:
            sys.meta_path.insert(0, self)
        if spec is None or spec.loader is None:
            return None
        _orig = spec.loader.exec_module
        _p = _P[name]

        def _exec(module, _orig=_orig, _p=_p):
            _orig(module)
            _p(module)

        spec.loader.exec_module = _exec
        return spec


sys.meta_path.insert(0, _F())
