# NAS versus NVMe: TP2 model startup on two DGX Sparks

Measurement date: 30 October 2026 (UTC)

This experiment answers how much additional server startup time NAS-backed model files caused on this two-node Spark configuration. All times below are measured locally; external recipe benchmarks are not used as loading results.

For **Qwen3.8-Flash-Next-NVFP4**, the NAS median was **1325.60 seconds**, versus **1190.66 seconds** with a local NVMe copy on each node: **134.94 seconds longer** (11.3% longer; 1.113× the NVMe time).

For **DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz**, the NAS median was **1268.10 seconds**, versus **413.76 seconds** with a local NVMe copy on each node: **854.34 seconds longer** (206.5% longer; 3.065× the NVMe time).

Each result uses three valid starts with both client OS caches cleared. The configured context window was 1,048,576 tokens, and every accepted start passed health and short inference checks. This does not certify inference with a million-token prompt.

## Results

| Model | Storage policy | Run 1 (s) | Run 2 (s) | Run 3 (s) | Median (s) | Extra vs local NVMe (s) | Ratio vs local NVMe |
|---|---|---:|---:|---:|---:|---:|---:|
| Qwen3.8-Flash-Next-NVFP4 | Both nodes: NAS (SMB/CIFS) | 1325.29 | 1325.60 | 1349.34 | 1325.60 | +134.94 | 1.113 |
| Qwen3.8-Flash-Next-NVFP4 | Main: NVMe; worker: main NVMe over read-only NFSv4.2 | 1176.55 | 1199.37 | 1197.82 | 1197.82 | +7.16 | 1.006 |
| Qwen3.8-Flash-Next-NVFP4 | Both nodes: their own NVMe copy | 1193.26 | 1190.66 | 1170.80 | 1190.66 | +0.00 | 1.000 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Both nodes: NAS (SMB/CIFS) | 1259.34 | 1268.10 | 1268.99 | 1268.10 | +854.34 | 3.065 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Main: NVMe; worker: main NVMe over read-only NFSv4.2 | 485.40 | 481.38 | 495.56 | 485.40 | +71.64 | 1.173 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Both nodes: their own NVMe copy | 415.57 | 413.76 | 411.14 | 413.76 | +0.00 | 1.000 |

## Engine loading segments

These are engine-reported durations, separate from the complete command-to-health interval. For each start the slower of the two observed TP ranks is selected, then its median across three starts is reported. Checkpoint-loader time and model-loader time overlap; they must not be added together or to the full startup interval. N/A means a complete two-rank observation was unavailable.

| Model | Storage policy | Median checkpoint-loader segment (s) | Median model-loader segment (s) |
|---|---|---:|---:|
| Qwen3.8-Flash-Next-NVFP4 | Both nodes: NAS (SMB/CIFS) | 456.27 | 471.46 |
| Qwen3.8-Flash-Next-NVFP4 | Main: NVMe; worker: main NVMe over read-only NFSv4.2 | 311.22 | 327.21 |
| Qwen3.8-Flash-Next-NVFP4 | Both nodes: their own NVMe copy | 293.05 | 307.72 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Both nodes: NAS (SMB/CIFS) | N/A | 1032.06 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Main: NVMe; worker: main NVMe over read-only NFSv4.2 | N/A | 252.62 |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | Both nodes: their own NVMe copy | N/A | 178.77 |

## Measurement method

The timer begins immediately before the main process executes `vllm serve` and ends at the first HTTP 200 response from `/health`. API polling uses a 0.2-second interval and a one-second request timeout. Ray/Compose startup before that command is outside this interval. Model initialization, checkpoint loading, kernel compilation/autotuning, KV allocation and API initialization after the command are included.

Before each start, the previous serving containers were removed and the cache-clearing helper succeeded on both hosts. Cache clearing precedes canonical preflight and Ray/Compose setup; any small configuration/header reads during preflight are outside the timer. Model copying and checksum verification had finished before comparison measurements began. Runtime mount observations confirm that each node used the selected read-only model source. Both nodes’ copies match the NAS payload by file name, size and SHA256.

Storage order was balanced: main/NFS → local/local → NAS/NAS in repetition 1; local/local → NAS/NAS → main/NFS in repetition 2; NAS/NAS → main/NFS → local/local in repetition 3. Each policy therefore occupies each position once. Calibration is excluded. Individual attempts, including failures, remain in the internal evidence vault.

Each host keeps its existing image fixed, and each model keeps the same model bytes, TP2 recipe and context setting across the three storage policies. Qwen uses resident n-gram embeddings and DeepSeek uses file-backed Engram tables through the image’s EXL3/Engram integration. The DeepSeek table path follows the selected model storage source; it is not staged independently on a fixed NVMe path. API readiness is not a claim that every byte of a file-backed lookup table is resident in RAM. DeepSeek can continue to use storage for Engram row lookups during inference; this experiment does not establish matched full inference throughput across the storage policies.

## Hardware and reproducibility

The measured hosts each have one NVIDIA GB10 GPU, approximately 121.7 GiB of unified host/GPU memory, and approximately 3.7 TiB of NVMe capacity. The worker in this experiment is not physically limited to a 1 TB disk. The main-only storage policy represents that deployment arrangement; it does not measure disk-capacity pressure.

Runtime payloads exclude Git history and download caches. Qwen comprises 25 files (132,735,004,591 bytes); the source repository HEAD was `fc694b54fb0174e0913e6adf86691ef85a4ead47`. DeepSeek comprises 56 files (357,486,881,279 bytes), with Hugging Face metadata revision `982b70452f399814f56b46272fd30394ae10d58c`.

The existing container image name is `easy-vllm:0.32.0-cu133-aarch64-source-sm12x`. Images were built independently on each host and have different image IDs and build-ledger identities. The observed vLLM source SHA, PyTorch build and driver agree across the nodes; each host’s image remains fixed across storage policies. The image name is not substituted for the observed package versions, listed below.

## Interpretation limits

These are client-cache-cleared application startup measurements. The NAS server cache was not cleared or controlled. A NAS backed by another protocol, network, disks or cache state may behave differently. The NAS used SMB/CIFS; the main-only share used NFSv4.2 over TCP. Thus the comparison measures these deployed storage policies, including their protocol and loader behavior, rather than an isolated physical-disk speed difference.

Fresh containers also incur kernel compilation and autotuning. The complete startup interval includes that work; engine-reported model-loading observations are retained separately so a storage change is not presented as the sole cause of every startup-time difference. Three repetitions provide a small sample; the individual values are reported instead of claiming a universal percentage.

## Serving configuration

Both recipes use TP2, eager execution, language-only serving and a runtime limit of one sequence. The configured context is 1,048,576 tokens. These are fixed experiment settings, not measured minimum memory requirements or an optimized concurrency limit.

| Model | KV dtype | Configured KV per GPU (GiB) | Max batched tokens | Speculative decoding |
|---|---|---:|---:|---|
| Qwen3.8-Flash-Next-NVFP4 | fp8_e4m3 | 14.0 | 2048 | Off |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | fp8 | 8.0 | 1536 | dspark, k=3 |

Qwen uses the tested YaRN factor-4 configuration, with the text maximum-position setting expanded to 1,048,576. DeepSeek uses its file-backed Engram source under the selected model root. The published hints retain the complete configuration.

## Observed node runtime

| Role | vLLM package metadata | PyTorch package metadata | Driver | vLLM source SHA |
|---|---|---|---|---|
| main | `0.32.1.dev0+g5df63a720.d20261029.cu133` | `2.13.0a0+9186a08b2c.nv26.7.59513937` | `580.178.04` | `5df63a7203cdb9328d824a27b682e77c7b9928be` |
| sub | `0.32.1.dev0+g5df63a720.d20261028.cu133` | `2.13.0a0+9186a08b2c.nv26.7.59513937` | `580.178.04` | `5df63a7203cdb9328d824a27b682e77c7b9928be` |

## Serving benchmark validation

A separate full benchmark was run on the representative per-node NVMe policy for each model. It is distinct from startup timing and does not establish equal inference throughput across storage policies. The preset uses synthetic 1,024-token inputs and 256-token outputs; the runtime sequence limit filters the sweep to concurrency 1. Full means a lite gate plus at least three GuideLLM repetitions. The benchmark reports retain actual counts, measurements and the applicable verdict; no throughput target was invented for this storage experiment.

| Model | Completed repetitions | Decode tokens/s (run order) | Performance verdict | Rubric authority |
|---|---:|---|---|---|
| Qwen3.8-Flash-Next-NVFP4 | 3 | 21.40 / 21.47 / 21.14 | PASS | weak |
| DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz | 3 | 22.26 / 23.38 / 23.38 | REFUTE | weak |

Performance verdicts are copied from the benchmark owner’s output and are separate from the measured storage-startup comparison. A REFUTE means the measured throughput fell below the selected performance reference; it does not turn a passed startup or inference check into a failed one.

For Qwen, no compatible external reference or user throughput target was selected. Its weak PASS uses the benchmark owner’s estimated achievable reference of 15.76 tokens/s, with a tolerance floor of 13.40 tokens/s; that reference is a modeled value, not a second measurement.

For DeepSeek, the weak performance rubric retains the previously identified author reference of 43.73 tokens/s for round-35 natural prose at concurrency 1. That recipe uses graphs and a performance-kernel bundle, while this fixed storage experiment uses eager execution, a minimal port and synthetic 1,024/256-token requests. Exact external checkpoint-byte equivalence was not established. The reference is a conservative external anchor, not a matched causal comparison or a user-specified throughput target. See the [author’s recipe](https://github.com/sfxnz/DeepSeek-V4.1-Flash-EXL3-vLLM-2x-DGX-Spark).

Detailed serving evidence and the published model hints accompany this report.

## Published reproducibility hints

One hint was published for each representative per-node NVMe recipe after independent fact checking. The remote tag object was checked against the local object. These hints retain the measured configuration and its limitations; they do not promise the same startup time on another storage system.

- **Qwen3.8-Flash-Next-NVFP4**: `hint/0.32.0-g5df63a7203cd/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8`.
  Payload commit: `ebc5d4910db98ea2d4ad567be18284aaf3d53544`.
- **DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz**: `hint/0.32.0-g5df63a7203cd/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8`.
  Payload commit: `29ce66c3786333792b69982f20192296da2c1139`.

