# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_서사.md`
- testlog: `../testlog/testlog_26091117_qwen38fn_22셀재수행_512k그룹완료체크포인트_판정.md`

## 서사

**증상**: 이 모델(Qwen3.8-Flash-Next)의 네이티브 max-model-len은 262144다. 이 셀은
context=524288(262144의 2배)를 요구했는데, YaRN rope 확장 인자 없이 트리플렛을 그대로 적용하면
로드 자체가 불가능하다(`testlog_26091117` §"R8 실서빙 재검증" 참조 — 직전 체크포인트의
`nv4-bf-512k-mmp`에서 최초로 이 증상을 만나 원인을 특정했다).

**원인**: 이 리포지토리의 R8 교정(`check_smoke_model.py` → `rope_scaling_translate.py`)이
로드 0초에 rope 확장 인자 부재를 지목하고 정확한 `hf-overrides` 병합 줄을 제시한다. 이 셀은
그 교정이 이미 검증된 이후의 재현 셀이므로, `output/multi/configs/nv4-f8-512k-mmp.yaml`에
`hf-overrides: '{"text_config":{"rope_parameters":{"mrope_interleaved":true,"mrope_section":
[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":2,
"original_max_position_embeddings":262144}}}'`를 처음부터 포함시켜 착수했다(`testlog_26091117`
표 #9).

**해소**: R8 오버라이드를 포함한 상태로 정상 로드·헬스 200·기능 스모크 통과. 5레벨(1/2/4/8/16)
벤치 전부 완주, decode t/s(동시성=1)=34.86, verdict=PASS(explore). kv_cache_dtype=fp8_e4m3와
PLE=mmap 조합에서도 YaRN factor=2 합성이 정상 동작함을 확인했다 — R8이 모델 변종(NVFP4)×KV
dtype(fp8) 축에서 재현됨을 보여주는 두 번째 증거다(첫 번째는 kv=auto 조합의 `nv4-bf-512k-mmp`).

**예산**: 이 조합(NVFP4 체크포인트 123.57GiB, PLE=mmap)의 예산 선판정 floor=40,478MiB로
여유가 있었다(가드 최소 8,192MiB 대비 32,286MiB 여유). PLE mmap이 weights_mib에서 48,828MiB를
감산하는 효과가 여기서도 그대로 적용됨을 확인했다.

## 되풀이하지 말 것

- **`hf-overrides`를 나중에 추가하지 말고 처음부터 넣어라**: 이 리포지토리의 512k+ 컨텍스트
  셀 전부(mmp 계열 4개)가 이 rope 확장 인자를 요구한다. `check_smoke_model.py`가 잡아주긴
  하지만, 그 STOP은 실 GPU 로드 직전에 발생하므로 미리 알고 있으면 시행착오 사이클을 아낀다.
- **같은 체크포인트의 `res`(PLE resident) 변종은 이 축과 별개로 실패한다** — NVFP4+resident+
  512k 조합은 이 셀과 같은 rope 오버라이드를 가지고 있어도 워치독에 사살된다(`nv4-bf-512k-res`·
  `nv4-f8-512k-res` 둘 다). PLE mmap이 성공의 전제조건이지 rope 오버라이드만으로는 부족하다 —
  둘을 혼동해 "rope만 맞으면 된다"고 가정하지 마라.
