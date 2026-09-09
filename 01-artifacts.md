# 1. 산출물 — 무엇이 실제로 쓰였나 (셀 b-768k-kvfp8-l1spec · 변종 spec7-graph0)

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1). `slot_confidence` 함께 읽어라.

| 슬롯 | 적용 |
|---|---|
| `triplet` | b-768k-kvfp8-l1spec.yaml / .sh / .env.b-768k-kvfp8-l1spec |
| `build_patch_post` | 10-deepgemm · 20-triton · 30-mxfp4 · 40-humming (빌드 로그 실측 적용) |
| `build_recipe` | Dockerfile · Dockerfile.source-build · requirements.txt |
| `compose` | docker-compose.yaml |

- **triplet** — 셀의 전부: fp8 KV(fp8_ds_mla로 강제 해소) · moe humming(auto는 MARLIN-repack OOM) · TP=2 ray · 10GiB 클램프(밸리 binding) · parser deepseek_v4.
- **build_patch_post** — 자동 적용됨. 본 셀은 stock 빌드지만 위 4종은 빌드 컨테이너에서 무조건 실행된 흔적이 빌드 로그에 있다.
- **build_recipe** — NGC 26.07(torch 2.13.0a0)×vLLM 0.29.0rc6 커플링, requirements constraint(0.28.0 baseline + flashinfer 0.6.18 양보).
- **compose** — master/slave Ray 오케스트레이션(NCCL/RoCE env 포함).
- runtime_patch/build_patch_pre/fork_pin은 이 셀에서 **미해당**(stock 0.29.0rc6 · 셀렉터 off).
