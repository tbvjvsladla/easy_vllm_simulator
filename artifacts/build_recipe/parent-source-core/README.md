# ⚠ generated-unverified — main 최종 attempt2 동결 입력과 관측 원장에서 조립한 새 local 재현안이며 미실행 · 생성: agent

# i5 main explicit parent closure — new reproduction, not executed

Main official source+POST05 build -> same-ID deps1 alias -> observed deps2 POST06 -> final i5 attempt2: cached PRE layer, native POST rerun/recompiled including75. This new reconstruction was not built or GPU-tested. Sub normal result is accepted through its own documents, not reconstructed or revalidated here.

Use artifacts/build_recipe as the working directory. Core context uses the unchanged five source files under parent-source-core/context. Intermediate deps2 uses exactly its Dockerfile and POST06 under parent-source-core/intermediate-deps2/context. Leaf context uses all24 final attempt2 files: existing19 selected recipe/script slots are reused, and missing5 are supplied under parent-source-core/leaf-support. No first failed attempt context or current active kit is substituted. Every source SHA is declared. Build independently on both nodes; do not transfer images.

## parent_source_core

```bash
# generated-unverified: new reproduction, not an observed invocation
: "${REPRO_CORE_TAG:?set an independent local core tag}"
docker pull nvcr.io/nvidia/pytorch@sha256:2140e699b3beaf7f96a0081fd9c9406bc3832b435cdb60dfa2d261f7d2f34a1c
docker tag nvcr.io/nvidia/pytorch@sha256:2140e699b3beaf7f96a0081fd9c9406bc3832b435cdb60dfa2d261f7d2f34a1c nvcr.io/nvidia/pytorch:26.07-py3
docker build --progress plain --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg TORCH_CUDA_ARCH=12.1 --build-arg BUILD_JOBS=8 -t "$REPRO_CORE_TAG" -f parent-source-core/context/Dockerfile.source-build parent-source-core/context
```

## alias

```bash
# generated-unverified: main observed same-ID alias; reproduce locally, no separate build
: "${REPRO_DEPS1_TAG:?set an independent local alias tag}"
docker tag "$REPRO_CORE_TAG" "$REPRO_DEPS1_TAG"
```

## intermediate_deps2

```bash
# generated-unverified: reconstructed deps2 intermediate from observed parent ledger args
: "${REPRO_INTERMEDIATE_TAG:?set independent deps2 tag}"
docker build --build-arg BASE_IMAGE="$REPRO_DEPS1_TAG" --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg SM12X_PORT=0 --build-arg SRC_DEPS_AUTHORITY=0 -t "$REPRO_INTERMEDIATE_TAG" -f parent-source-core/intermediate-deps2/context/Dockerfile.source-build-deps2 parent-source-core/intermediate-deps2/context
```

## leaf_sm12x

```bash
# generated-unverified: attempt2 source context assembled from existing slots plus five support files; selected Dockerfile may include its recorded warning
mkdir -p leaf-sm12x/context/build_patches leaf-sm12x/context/build_patches_src
cp Dockerfile.source-build-sm12x leaf-sm12x/context/Dockerfile
cp parent-source-core/leaf-support/build_patches/.gitkeep leaf-sm12x/context/build_patches/.gitkeep
cp ../build_patch_post/05-ngc-source032-deps-compat.sh leaf-sm12x/context/build_patches/05-ngc-source032-deps-compat.sh
cp ../build_patch_post/06-b12x-optional032.sh leaf-sm12x/context/build_patches/06-b12x-optional032.sh
cp ../build_patch_post/70-exllamav3-build.sh leaf-sm12x/context/build_patches/70-exllamav3-build.sh
cp ../build_patch_post/72-exl3-row-store.sh leaf-sm12x/context/build_patches/72-exl3-row-store.sh
cp ../build_patch_post/73-vllm-exl3-build.sh leaf-sm12x/context/build_patches/73-vllm-exl3-build.sh
cp ../build_patch_post/74-exl3-moe-prep-fused.sh leaf-sm12x/context/build_patches/74-exl3-moe-prep-fused.sh
cp ../build_patch_post/75-exl3-python-deps032.sh leaf-sm12x/context/build_patches/75-exl3-python-deps032.sh
cp ../build_patch_post/79-exl3-port-selfcheck.sh leaf-sm12x/context/build_patches/79-exl3-port-selfcheck.sh
cp parent-source-core/leaf-support/build_patches/Dockerfile.source-build-deps2 leaf-sm12x/context/build_patches/Dockerfile.source-build-deps2
cp parent-source-core/leaf-support/build_patches_src/.gitkeep leaf-sm12x/context/build_patches_src/.gitkeep
cp ../build_patch_pre/80-exl3-quant-method.sh leaf-sm12x/context/build_patches_src/80-exl3-quant-method.sh
cp ../build_patch_pre/81-exl3-model-wiring.sh leaf-sm12x/context/build_patches_src/81-exl3-model-wiring.sh
cp ../build_patch_pre/82-exl3-h2d-stage.sh leaf-sm12x/context/build_patches_src/82-exl3-h2d-stage.sh
cp ../build_patch_pre/83-engram-file-backend.sh leaf-sm12x/context/build_patches_src/83-engram-file-backend.sh
cp ../build_patch_pre/84-sm12x-block-shim.sh leaf-sm12x/context/build_patches_src/84-sm12x-block-shim.sh
cp ../build_patch_pre/85-exl3-serial-streams.sh leaf-sm12x/context/build_patches_src/85-exl3-serial-streams.sh
cp ../build_patch_pre/86-exl3-worker-hooks.sh leaf-sm12x/context/build_patches_src/86-exl3-worker-hooks.sh
cp ../build_patch_pre/87-engram-embed-skip.sh leaf-sm12x/context/build_patches_src/87-engram-embed-skip.sh
cp ../build_patch_pre/88-engram-prestage.sh leaf-sm12x/context/build_patches_src/88-engram-prestage.sh
cp ../build_patch_pre/89-kv-null-block-zero.sh leaf-sm12x/context/build_patches_src/89-kv-null-block-zero.sh
cp parent-source-core/leaf-support/build_patches_src/PROVENANCE.json leaf-sm12x/context/build_patches_src/PROVENANCE.json
cp parent-source-core/leaf-support/build_patches_src/exl3_port_lib.py leaf-sm12x/context/build_patches_src/exl3_port_lib.py
: "${REPRO_IMAGE_TAG:?set independent i5 leaf tag}"
docker build --build-arg BASE_IMAGE="$REPRO_INTERMEDIATE_TAG" --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg TORCH_CUDA_ARCH=12.1a --build-arg BUILD_JOBS=4 --build-arg SM12X_PORT=1 -t "$REPRO_IMAGE_TAG" -f leaf-sm12x/context/Dockerfile leaf-sm12x/context
```

Main final native POST ran after adding75; only PRE build layer was cached. GPU primitive evidence is historical; no new build, accuracy, TP2 serving or benchmark is established by this reconstruction. The sub has its own historical dependency/build path; no duplicate sub inspection is performed. Original full main i5 build argv is docs/logs/multi4_26102919/i5/main-variant-build-command.txt. Intermediate full CLI is unavailable, so its command is reconstructed from observed parent ledger arguments.

The 24-file source inventory hashes identify the raw final-attempt2 inputs validated before annotation. The selected Dockerfile in this kit can include the official generated-unverified warning after its syntax directive; its distributed bytes therefore differ only by that recorded warning. The build_recipe file_records frozen_source_sha256 identifies the raw original and sha256 identifies the annotated distributed file. The 18 patch scripts and five support files remain raw byte-identical. Reconstructed commands use the annotated Dockerfile and do not establish a new execution.
