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

ARG VLLM_VERSION=0.19.1
ARG CUDA_VERSION=130

RUN cp /etc/pip/constraint.txt /etc/pip/constraint.txt.bak \
    && : > /etc/pip/constraint.txt

RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_35_${CPU_ARCH}.whl"

COPY requirements.txt /tmp/requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    pip install -r /tmp/requirements.txt

WORKDIR /workspace

EXPOSE 8000