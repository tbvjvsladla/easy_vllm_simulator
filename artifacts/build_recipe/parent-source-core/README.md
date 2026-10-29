# ⚠ generated-unverified — 명시 parent manifest와 main 실행/alias/leaf 원장에서 재구성한 새 양노드 local-build 예시이며 실행하지 않았다 · 생성: agent

# Main parent source core → same-ID alias → deps2 leaf

This is a new reproduction plan. It was not executed. Perform each local build on each node; never transfer built images. The original main core Dockerfile and all five frozen context files remain byte-for-byte unchanged.

Main observed history: official source core + POST05 was built, deps1 was a same-ID alias (build_performed=false), deps2 added POST06. Sub historical history differs: an older source then a real deps1 repair then deps2; its older core Dockerfile was not recovered. This complete main path is proposed for NEW local builds on both nodes and is not evidence that sub historically executed the main path.

Place all collected parent-source-core files under artifacts/build_recipe/parent-source-core. Use artifacts/build_recipe as the working directory. The core context is parent-source-core/context and contains exactly Dockerfile.source-build, requirements.txt, build_patches/05-ngc-source032-deps-compat.sh, build_patches/.gitkeep, build_patches_src/.gitkeep. PRE contains only the empty scaffold: no i5 modules. Keep alias/command/ledger/README/manifest metadata outside context/.

For the leaf context, put the unchanged selected Dockerfile.source-build-deps2 in this working directory and copy only artifacts/build_patch_post/06-b12x-optional032.sh to build_patches/06-b12x-optional032.sh. Do not merge current i5 PRE/POST. The existing collector may also supply the exact COPY input in build_recipe; those bytes must agree.

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

## leaf

```bash
# generated-unverified: reconstructed from selected leaf ledger, original full CLI unavailable
: "${REPRO_IMAGE_TAG:?set an independent local leaf tag}"
mkdir -p build_patches
cp ../build_patch_post/06-b12x-optional032.sh build_patches/06-b12x-optional032.sh
docker build --build-arg BASE_IMAGE="$REPRO_DEPS1_TAG" --build-arg VLLM_REPO=https://github.com/vllm-project/vllm.git --build-arg VLLM_REF=5df63a7203cdb9328d824a27b682e77c7b9928be --build-arg SM12X_PORT=0 --build-arg SRC_DEPS_AUTHORITY=0 -t "$REPRO_IMAGE_TAG" -f Dockerfile.source-build-deps2 .
```

Core argv is preserved in core-build-command.txt. Alias facts are preserved in alias-evidence.json. Leaf build arguments are observed in leaf-ledger-evidence.json, but no original full leaf CLI was recovered, so its command is reconstructed. Base digest is an explicit observed NGC pin; source FROM bytes are unchanged. Neither new per-node ABI equality nor serving success is established by this README.
