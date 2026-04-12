# syntax=docker/dockerfile:1

FROM nvcr.io/nvidia/pytorch:26.01-py3

ARG VLLM_VERSION=0.19.0
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
