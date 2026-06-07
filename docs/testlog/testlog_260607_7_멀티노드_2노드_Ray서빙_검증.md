# testlog 260607-7 — 멀티노드 2노드 Ray 서빙 검증 (gpt-oss-120b-MXFP4 / 0.18.0)

## 목적
양 노드 multi-node 브랜치 0.18.0 셋업 후, 실제 2노드 Ray 클러스터 분산 서빙 + multi-smoke 검증.
계획: docs/plan/plan_260607_6. Seed seed_8f97998bdec0.

## 절차 / 결과 (전부 통과)
1. **빌드 (양 노드, 병렬 background)**: 메인 master + 서브 slave 멀티노드 0.18.0 이미지 독립 빌드.
   - 메인 `vllm_serving_server-vllm-master-serve` (exit 0, 32.8GB), 서브 `...-slave-serve` (exit 0, 21.6GB). NGC 26.01 캐시 활용. ✅
2. **Ray 클러스터 기동**:
   - 메인 `docker compose --env-file envs/.env.gpt-oss-120b-MXFP4 --profile master up -d` → Ray head + slave 합류 대기.
   - 서브 `--profile slave up -d` → master Ray head 합류(worker).
   - serve_runner.sh 로그: `[master] Slave node connected. Cluster ready with 2 GPUs.` ✅
   - `ray status`: 2 nodes, 0.0/2.0 GPU (양 노드 합류 확인).
3. **2노드 분산 로드**: gpt-oss-120b(120B MXFP4) 15 safetensors shards 로드.
   - `RayWorkerWrapper pid=427, ip=192.168.100.11`(서브 노드) 참여 → **진짜 2노드 tensor-parallel 확인**. ✅
   - 엔드포인트 :9000 health http 200 (~110s 로드 완료).
4. **multi-smoke (master :9000)**:
   - #1 "한 문장 자기소개" → 80 tokens 생성(reasoning, content=null은 max_tokens 소진 — 모델 정상 작동).
   - #2 "2+2는? 숫자만" → **content: "4"** (finish_reason: stop, 52 tokens) → **합격** ✅
5. **정리**: 양 노드 master/slave down (컨테이너 제거).

## 결론
**멀티노드 2노드 Ray 분산 서빙 end-to-end 검증 완료.** master(메인 192.168.100.10)+slave(서브 192.168.100.11)
ConnectX-7 Ray 클러스터에서 gpt-oss-120b(0.18.0 prebuilt)가 정확한 추론("4") 생성.
빌드(양 노드 병렬)→Ray 클러스터→분산 로드→추론 전 경로 동작.

## 검증된 전체 토대 (세션 종합)
네트워크·SSH → 코드이식(sync_to_sub.sh) → CC↔CC 통신(claude -p) → 빌드 교차검증 →
빌딩블럭 gitignore 일관성 → 멀티노드 0.18.0 양노드 셋업 → **2노드 Ray 서빙 + multi-smoke**. 전부 실증.

## 비고
- gpt-oss는 reasoning 모델 → 스모크 시 max_tokens 충분히(content 확인하려면 finish_reason=stop까지).
- 메인 0.18.0 멀티노드 패치는 검증 통과 → smoke-before-push 충족(커밋/푸시는 사람 승인 시).
