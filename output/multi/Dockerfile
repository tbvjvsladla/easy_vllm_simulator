# syntax=docker/dockerfile:1

FROM nvcr.io/nvidia/pytorch:26.01-py3

# 시스템 레벨 네트워크 디버깅 도구
# - iproute2: ip 명령어 (ip addr, ip route)
# - netcat-openbsd: nc 명령어 (포트 연결 체크)
# - iputils-ping: ping
# - dnsutils: nslookup, dig
RUN apt-get update && apt-get install -y --no-install-recommends \
        iproute2 \
        netcat-openbsd \
        iputils-ping \
        dnsutils \
    && rm -rf /var/lib/apt/lists/*

ARG VLLM_VERSION=0.18.0
ARG CUDA_VERSION=130

RUN cp /etc/pip/constraint.txt /etc/pip/constraint.txt.bak \
    && : > /etc/pip/constraint.txt

RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_35_${CPU_ARCH}.whl"

COPY requirements.txt /tmp/requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r /tmp/requirements.txt

# [멀티노드 전용] ray — 2노드 Ray 분산 서빙(serve_runner.sh 의 ray start). vLLM 0.18.0 reqs 미선언 →
#   명시 설치(prebuilt wheel 은 --no-deps 라 더더욱 안 들어옴). netcat 등 네트워크 도구는 위 apt 에서 설치됨.
ARG RAY_VERSION=2.48.0
RUN pip install ray==${RAY_VERSION}

WORKDIR /workspace

EXPOSE 8000