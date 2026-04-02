# bash 스크립트에 gpt oss 120b를 구동하기 위한 명령어 절차 및 로그

## 환경변수 등록
```bash
root@dfdc0ebab019:/# export TIKTOKEN_ENCODINGS_BASE=/encodings
root@dfdc0ebab019:/# export TIKTOKEN_RS_CACHE_DIR=/encodings
```
## vllm-serve 서빙명령어
```bash
root@dfdc0ebab019:/# vllm serve /app/models/gpt-oss-120b \
  --served-model-name gpt-oss-120b \
  --host 0.0.0.0 \
  --port 8000 \
  --gpu-memory-utilization 0.95 \
  --max-model-len 65536 \
  --tool-call-parser openai \
  --enable-auto-tool-choice \
  --reasoning-parser openai_gptoss
```

## 서빙 실행 성공 로그
```bash
(APIServer pid=861) INFO:     Started server process [861]
(APIServer pid=861) INFO:     Waiting for application startup.
(APIServer pid=861) INFO:     Application startup complete.
(APIServer pid=861) INFO:     127.0.0.1:35206 - "GET /v1/models HTTP/1.1" 200 OK
```
## 다른 터미널 -> 컨테이너 bash쉘 접속 -> 로그 확인
```bash
ash@ash:~/ws_docker/vllm_serving_server$ docker exec -it vllm_server bash
root@dfdc0ebab019:/workspace# curl http://localhost:8000/v1/models
{"object":"list","data":[{"id":"gpt-oss-120b","object":"model","created":1775029496,"owned_by":"vllm","root":"/app/models/gpt-oss-120b","parent":null,"max_model_len":65536,"permission":[{"id":"modelperm-bfeb81a25ff427f8","object":"model_permission","created":1775029496,"allow_create_engine":false,"allow_sampling":true,"allow_logprobs":true,"allow_search_indices":false,"allow_view":true,"allow_fine_tuning":false,"organization":"*","group":null,"is_blocking":false}]}]}root@dfdc0ebab019:/workspace#
```
## 다른터미널에서 통신테스트
```bash
ash@ash:~/ws_docker/vllm_serving_server$ curl http://localhost:7900/v1/models
{"object":"list","data":[{"id":"gpt-oss-120b","object":"model","created":1775029656,"owned_by":"vllm","root":"/app/models/gpt-oss-120b","parent":null,"max_model_len":65536,"permission":[{"id":"modelperm-b847df9a5a4726c7","object":"model_permission","created":1775029656,"allow_create_engine":false,"allow_sampling":true,"allow_logprobs":true,"allow_search_indices":false,"allow_view":true,"allow_fine_tuning":false,"organization":"*","group":null,"is_blocking":false}]}]}ash@ash:~/ws_docker/vllm_serving_server$
```