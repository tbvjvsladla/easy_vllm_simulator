#!/usr/bin/env python3
"""SHIM — topology_parity 의 정본은 `.claude/policies/runtime/topology_parity.py` 다.

plan_26093022 Step 1(2단 이관의 1단): 4자일치 술어는 pre-commit·selftest·린트가 부르는 **기초층 런타임**이라
스킬 scripts/ 에서 policies/runtime/ 으로 올라갔다. 브랜치 싱크는 경계를 스스로 넘지 못하므로(옛 경로를 부르는
체크아웃이 한 싱크 동안 남는다) 이 위임 shim 을 한 번 둔다. **다음 브랜치 싱크(Step 8)에서 삭제한다** —
이 파일에 로직을 더하지 않는다.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

_CANON = Path(__file__).resolve().parents[4] / ".claude/policies/runtime/topology_parity.py"
_spec = importlib.util.spec_from_file_location("_topology_parity_canonical", _CANON)
if _spec is None or _spec.loader is None or not _CANON.is_file():
    raise ImportError(f"topology_parity canonical module missing: {_CANON}")
_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_module)
globals().update({k: v for k, v in vars(_module).items() if not k.startswith("__")})

if __name__ == "__main__":
    sys.exit(_module.main())
