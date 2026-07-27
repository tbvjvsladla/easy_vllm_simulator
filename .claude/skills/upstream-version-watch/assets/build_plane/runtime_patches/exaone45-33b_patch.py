# exaone45-33b_patch.py — EXAONE-4.5 image/video processor compat shim (런타임 패치, 휘발·비추적).
#
# WHAT : transformers 의 auto processor 이름해소기를 patch — 모델파일이 선언한 포크-시대 이름
#        Exaone4_5_Image/VideoProcessor* → 릴리스 transformers 의 Qwen2VL* 클래스로 매핑.
# WHY  : EXAONE-4.5-33B 는 멀티모달(VL)인데 이미지의 transformers 5.12.1 에 그 processor 클래스가
#        부재(모델파일은 포크-시대 명시이름 선언). 전처리 스키마는 Qwen2-VL 과 동일 → 별칭이 정합.
# REF  : 외부검색 — HF LGAI-EXAONE/EXAONE-4.5-33B · fork lkm2835/vllm · nuxlear/transformers.
#        (헌법 「모델구동 런타임 패치 따름정리」 — 참조-그라운디드 생성, 환경·bump 마다 재유도.)
# RETIRE-WHEN : transformers 가 exaone4_5 image/video processor 클래스를 정식 머지하면 불필요.
# SITE-INIT SAFE : 최상위에서 transformers 를 import 하지 않는다(huggingface_hub 순환 회피).
#        meta-path finder 로 auto 모듈 로드 *직후* 패치 → engine + 모든 Ray worker 프로세스에 적용.
import sys, importlib.util, importlib.abc


def _patch_image(mod):
    try:
        from transformers import Qwen2VLImageProcessor as QI
        try:
            from transformers.models.qwen2_vl.image_processing_qwen2_vl_pil import Qwen2VLImageProcessorPil as QIP
        except Exception:
            QIP = QI
        _o = mod.get_image_processor_class_from_name
        mod.get_image_processor_class_from_name = lambda n, _o=_o: (
            (QIP if n.endswith("Pil") else QI)
            if (n and n.startswith("Exaone4_5_ImageProcessor")) else _o(n))
        sys.stderr.write("[exaone45-patch] image resolver patched\n")
    except Exception as e:
        sys.stderr.write("[exaone45-patch] img warn: %s\n" % e)


def _patch_video(mod):
    try:
        from transformers import Qwen2VLVideoProcessor as QV
        _o = mod.video_processor_class_from_name
        mod.video_processor_class_from_name = lambda n, _o=_o: (
            QV if (n and n.startswith("Exaone4_5_VideoProcessor")) else _o(n))
        sys.stderr.write("[exaone45-patch] video resolver patched\n")
    except Exception as e:
        sys.stderr.write("[exaone45-patch] vid warn: %s\n" % e)


_P = {"transformers.models.auto.image_processing_auto": _patch_image,
      "transformers.models.auto.video_processing_auto": _patch_video}


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
