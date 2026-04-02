# devlog 250402 — Qwen3.5-35B-A3B 서빙 설정 구축 및 vLLM 버그 패치

## 작업 요약

Qwen3.5-35B-A3B 모델 서빙을 위해 vLLM 0.18.0 → 0.18.1 업그레이드를 시도했으나, `transformers` 라이브러리와의 호환성 버그가 양쪽 버전 모두에 존재함을 확인했다. 런타임 패치를 sh 파일에 내장하는 방식으로 해결하고 서빙에 성공했다.

---

## 1. 문제: vLLM 0.18.0에서 Qwen3.5 로딩 실패

```
huggingface_hub.errors.StrictDataclassClassValidationError:
  Class validation error for validator 'validate_rope':
    TypeError: unsupported operand type(s) for -=: 'set' and 'list'
```

**원인 분석:**
- `vllm/transformers_utils/configs/qwen3_5_moe.py:78`에서 `ignore_keys_at_rope_validation`을 `list`로 정의
- `transformers/modeling_rope_utils.py:919`의 `_check_received_keys`에서 `received_keys -= ignore_keys` 실행 시, `received_keys`는 `set`이고 `ignore_keys`는 `list`여서 TypeError 발생

**해당 코드:**
```python
# qwen3_5_moe.py:78 (버그)
kwargs["ignore_keys_at_rope_validation"] = [    # list
    "mrope_section",
    "mrope_interleaved",
]
```

---

## 2. vLLM 0.18.1 업그레이드 시도 — 동일 버그 확인

Dockerfile의 `ARG VLLM_VERSION=0.18.1`로 변경 후 재빌드했으나, 동일한 에러가 재현되었다. vLLM 0.18.1에서도 `qwen3_5_moe.py`의 해당 코드가 수정되지 않았음을 확인.

---

## 3. 해결: 런타임 패치 (list → set)

컨테이너 내부에서 직접 패치하여 동작을 확인한 후, sh 파일에 패치 로직을 내장하는 방식을 채택했다.

**패치 내용:**
```python
# 수정 후 (set으로 변경)
kwargs["ignore_keys_at_rope_validation"] = {    # set
    "mrope_section",
    "mrope_interleaved",
}
```

**수동 패치 명령 (컨테이너 내부에서 검증 시 사용):**
```bash
python -c "
p='/usr/local/lib/python3.12/dist-packages/vllm/transformers_utils/configs/qwen3_5_moe.py'
t=open(p).read()
t=t.replace('validation\"] = [','validation\"] = {',1)
t=t.replace('\"mrope_interleaved\",\n        ]','\"mrope_interleaved\",\n        }',1)
open(p,'w').write(t)
print('done')
"
```

---

## 4. sh 파일에 idempotent 패치 내장

`configs/Qwen3.5-35B-A3B-normal.sh`에 패치 로직을 포함시켰다. `grep`으로 미패치 상태를 확인한 뒤에만 적용하여 중복 적용을 방지한다.

```bash
# 패치 적용 여부 판단
PATCH_TARGET="/.../qwen3_5_moe.py"
if [ -f "$PATCH_TARGET" ] && grep -q 'validation"\] = \[' "$PATCH_TARGET"; then
    # python으로 list → set 변환
    echo "[patch] qwen3_5_moe.py: ignore_keys_at_rope_validation list -> set"
else
    echo "[patch] qwen3_5_moe.py: already patched or not found, skipping"
fi
```

**검증 결과:**
- 1회차 실행: `[patch] qwen3_5_moe.py: ignore_keys_at_rope_validation list -> set` → 패치 적용
- 2회차 실행: `[patch] qwen3_5_moe.py: already patched or not found, skipping` → 중복 적용 없이 스킵

---

## 5. `docker compose build` 명령어 문제 발견 및 수정

### 문제

`docker compose build`(프로필 미지정)를 실행하면 아무것도 빌드되지 않는 현상 발생.

**원인:** 모든 서비스(`vllm`, `vllm-serve`)에 `profiles`가 설정되어 있어, `--profile` 없이 실행하면 빌드 대상이 0개.

```bash
# docker compose config 출력
services: {}    # ← 비어있음
```

### 해결

CLAUDE.md와 README.md의 재빌드 명령어를 수정:

```bash
# 수정 전
docker compose build

# 수정 후
docker compose --profile debug build
```

---

## 6. WSL2 포트 예약 이슈

`SERVING_PORT=7910`으로 설정 시 포트 바인딩 실패:

```
Error: listen tcp 0.0.0.0:7910: bind: An attempt was made to access a socket
in a way forbidden by its access permissions.
```

**원인:** WSL2에서 Windows Hyper-V가 특정 포트 범위를 예약하여 사용 불가.
**해결:** 사용 가능한 포트로 변경하여 서빙 성공.

---

## 7. 서빙 성공 확인

Qwen3.5-35B-A3B 모델 서빙 및 API 통신 테스트 완료. "안녕 너는 누구야?" 질의에 정상 응답 확인.

---

## 8. 최종 파일 현황

```
vllm_serving_server/
├── Dockerfile                          # VLLM_VERSION=0.18.1로 업데이트
├── docker-compose.yaml
├── requirements.txt
├── envs/
│   ├── .env.gpt-oss-120b-normal
│   └── .env.Qwen3.5-35B-A3B-normal    ← NEW
├── configs/
│   ├── gpt-oss-120b-normal.sh
│   ├── gpt-oss-120b-normal.yaml
│   ├── Qwen3.5-35B-A3B-normal.sh      ← NEW (런타임 패치 포함)
│   └── Qwen3.5-35B-A3B-normal.yaml    ← NEW
└── docs/devlog/
    └── devlog_250402_1_...             ← 이 문서
```

---

## 9. 향후 참고사항

- 이 패치는 vLLM이 `ignore_keys_at_rope_validation`을 `set`으로 수정하면 불필요해진다
- sh 파일의 패치 로직이 `grep`으로 미패치 상태를 확인하므로, vLLM 업그레이드 시 자동으로 스킵된다
- vLLM 업그레이드 시에는 패치 대상 파일 경로(`/usr/local/lib/python3.12/...`)가 Python 버전에 의존하므로 확인 필요
