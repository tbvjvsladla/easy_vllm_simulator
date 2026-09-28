# 커널 7.0.0-1019-nvidia NCCL RoCE 회귀 검증 결과

> 대상: DGX Spark(GB10) 2노드 이상으로 이 저장소의 multi-node 분산 서빙을 운영하는 배포자
> 상태: 검증 완료 · 이 구성에서 **회귀 미재현** · 해당 서빙 상주 운영 중
> 발행 시점: 2026-09-28 10 KST

## 한 줄 결론

커널 `7.0.0-1019-nvidia`에서 NCCL RoCE가 `ibv_reg_mr_iova2` ENOMEM으로 깨진다는 보고(NVIDIA 포럼 383023)를 GB10 2노드에서 재현하려 했지만,
**모델리스 NCCL 시험과 DeepSeek-V4-Flash-0731 1M context 분산 서빙 모두 RoCE로 정상 동작했습니다.** 이 구성에서는 커널 다운그레이드가 필요하지 않습니다.
단, 아래 **§4 판정 범위**를 벗어나는 구성(특히 GPU Direct RDMA가 켜지는 경우)은 직접 확인해야 합니다.

## 1. 보고된 문제

| 항목 | 포럼 보고 |
|---|---|
| 증상 | 모델 로드는 되지만 프로파일링·통신 초기화에서 `Call to ibv_reg_mr_iova2 failed with error Cannot allocate memory` · `NVRM ... NV_ERR_NO_MEMORY` · `NCCL error: unhandled system error` |
| 실패 커널 | `7.0.0-1019-nvidia` |
| 정상 커널 | `6.17.0-1032-nvidia` |
| 지목된 원인 | 새 커널에서 `CmaTotal: 0 kB`(CMA 예약 없음) |
| NVIDIA 공지 | 조사 중 · 업데이트 보류 권고 |
| 커뮤니티 우회 | `NCCL_IB_DISABLE=1`(TCP · 처리량 저하) 또는 6.17 롤백 + `apt-mark hold` |

## 2. 원인 조건은 실재한다

양 노드(커널 7.0.0-1019)에서 확인했습니다.

| 항목 | 6.17.0-1031 | 7.0.0-1019 |
|---|---|---|
| `/boot/config-*` `CONFIG_CMA_SIZE_MBYTES` | 128 | **0** |
| `/proc/meminfo` `CmaTotal` | — | **0 kB** |
| CUDA `DMA_BUF_SUPPORTED` / `GPU_DIRECT_RDMA_SUPPORTED` | 미측정 | **0 / 0** |

## 3. 검증 결과

### 3.1 모델리스 2노드 NCCL all-reduce (1 MiB ~ 1 GiB)

| 조건 | 결과 | 1 GiB busbw |
|---|---|---|
| 내장 IB 기본값 | PASS · `NET/IB` | 21.92 GiB/s |
| GDR·DMA-BUF 강제 켬 | PASS(NCCL이 GDR을 스스로 끔) | 21.95 |
| HPC-X IBext 플러그인 | PASS | 21.89 |
| 메모리 압박(가용 ~14 GiB → ~9 GiB) | PASS · ENOMEM 없음 | 21.21–21.87 |
| `NCCL_CUMEM_ENABLE=0`(vLLM 조건) | PASS | 21.81 |

21.9 GiB/s는 약 176 Gb/s로 링크 한계에 가깝습니다. 모든 조건에서 `iova2`·`Cannot allocate`·`NVRM` 오류는 0건이었습니다.

### 3.2 실제 분산 서빙 — DeepSeek-V4-Flash-0731

| 항목 | 값 |
|---|---|
| 구성 | vLLM 0.29.0rc6 · TP=2(Ray) · max-model-len 1,048,576 · fp8 KV · dspark 추측 디코딩 k=7 · cudagraph |
| NCCL 전송 | `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` → 양 노드 `Using network IB`(RoCE) |
| 포럼 실패 구간(로드 뒤 프로파일링·KV 할당) | 통과 · KV 1,941,478 tokens |
| `iova2` · ENOMEM · `NV_ERR` · dmesg NVRM/mlx5 오류 | 0 |
| READY | 약 1,035초 |
| 기능 스모크 | PASS(health 200 + 추론) |
| full bench(GuideLLM · 동시성 1/2/4/8/16 × 반복 3) | 완주 · 절삭 0 · 동시성 1 decode **32.2 t/s**(재현 밴드 3.15%) |
| 커널 6.17 시절 같은 레시피(2026-09-09) | 30.54 t/s — 7.0에서 성능 저하 없음 |

## 4. 판정 범위와 해석

- **범위**: GB10 ×2 · 드라이버 580.173.02 · 이미지 `easy-vllm:0.29.0rc6-cu133-aarch64-source`(NCCL 2.30.7) · NCCL 내장 verbs 경로 · 위 레시피.
- **미재현의 가장 유력한 이유(추정)**: 현 드라이버가 GB10에 대해 DMA-BUF/GDR 미지원을 보고해 NCCL이 GDR을 켜지 않습니다(`GDR 0`).
  포럼 실패의 `NV_ERR_NO_MEMORY`는 GPU 메모리 등록 쪽이므로, 이 구성은 그 경로를 애초에 타지 않는 것으로 봅니다. 6.17에서 같은 속성을 재지 않아 대조는 미완입니다.
- 09-09와의 비교에서 달라진 것은 커널만이 아닙니다. 이미지가 재빌드됐고 NCCL env 일부(`NCCL_DMABUF_ENABLE=0` 등 GDR 계열 선언값)가 다릅니다. 양쪽 모두 `GDR 0`이라 동작에는 닿지 않았을 것으로 추정합니다.

## 5. 배포자 조치

### 5.1 내 노드 확인

```bash
uname -r                                        # 7.0.0-1019-nvidia 인가
grep -i cma /proc/meminfo                       # CmaTotal: 0 kB 이면 원인 조건 있음
grep CONFIG_CMA_SIZE_MBYTES /boot/config-$(uname -r)
```

### 5.2 RoCE로 서빙하려면 (이 저장소 · multi-node 브랜치)

1. `output/multi/manifest.yaml`의 `interconnect:` 아래에 `nccl_transport: rdma`를 적습니다. 키가 없으면 종전대로 `socket`(TCP)입니다.
2. `render_dockerfile.py --nccl-envfile --manifest output/multi/manifest.yaml --out output/multi/envs/.env.interconnect`로 다시 생성하고,
   `sync_to_sub.sh`로 서브에 배달합니다. 생성물을 손으로 고치면 서브에 전달되지 않습니다(배달 시 재렌더).
3. 기동 후 **엔진 로그에서 `Using network IB`를 반드시 확인**합니다. 설정 이름이 rdma여도 로그가 `Using network Socket`이면 RoCE가 아닙니다.
4. `ibv_reg_mr_iova2` · `Cannot allocate memory` · `NV_ERR_NO_MEMORY`가 보이면 회귀 재현입니다. `socket`으로 되돌려 서빙을 유지하고 롤백을 검토합니다.

### 5.3 재현될 경우의 롤백 (포럼 권고)

`/boot`에 6.17 커널이 남아 있으면 재설치 없이 grub 기본값만 바꿉니다.

```bash
sudo sed -i 's|^#\?GRUB_DEFAULT=.*|GRUB_DEFAULT="Advanced options for DGX OS GNU/Linux>DGX OS GNU/Linux, with Linux 6.17.0-1032-nvidia"|' /etc/default/grub
sudo update-grub && sudo reboot
sudo apt-mark hold linux-nvidia-hwe-24.04 linux-image-nvidia-hwe-24.04 linux-headers-nvidia-hwe-24.04
```

## 6. 저장소 변경

| 커밋 | 내용 |
|---|---|
| `2e91a1e` · `8319b4f` · `ab1fade` | `manifest.interconnect.nccl_transport`(socket 기본 · rdma = `IB_DISABLE=0`·`NET=IB`)로 NCCL 전송을 선택. 종전에는 Socket이 불변값으로 고정돼 RoCE로 돌릴 정식 경로가 없었습니다. |
| `c241e8f` · `c38cc72` | multi 캠페인 종결 게이트와 증거 포인터 연산 교정 |

⚠ `2e91a1e`의 메시지는 09-09 조건을 `IB_DISABLE=1`로 잘못 적었습니다. 실제로 09-09는 `IB_DISABLE=0`·`Using network IB`였고, 주석은 `ab1fade`에서 정정했습니다.

## 7. 참조

- hint 태그: `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plenone-spec7-graph`
  (재현 키트 · 벽 지도 · full bench. 이 태그의 서빙 설정에는 `enable-auto-tool-choice`가 없어 tool 호출 에이전트에 쓰려면 한 줄을 더해야 합니다)
- 외부: NVIDIA Developer Forums 383023 "DGX Spark regression: kernel 7.0.0-1019-nvidia causes NCCL RoCE ibv_reg_mr_iova2 ENOMEM, 6.17.0-1032 works"
