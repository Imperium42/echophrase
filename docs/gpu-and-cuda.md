# GPU backends (and why there is no CUDA)

| Platform | Execution provider | Requirements |
|----------|--------------------|--------------|
| Windows | **DirectML** | Any DirectX 12 GPU (NVIDIA, AMD, Intel, Qualcomm). Ships with Windows, nothing to install. Falls back to CPU. |
| macOS | Metal / CoreML | Apple Silicon (M1+), macOS 13.3+ |
| Copilot+ PCs | NPU (auto) | Snapdragon X or Intel Core Ultra, auto-selected by ONNX Runtime |
| Linux / WSL | CPU | Nothing extra. GPU acceleration is not supported today. |

## Why no CUDA?

Echophrase used to ship CUDA execution providers on Linux/WSL and as an
optional Windows override. That path was removed because the install matrix was
a constant source of "works on my machine" breakage:

- **Driver / Toolkit / cuDNN triangulation** - every install needed a matching
  NVIDIA driver, CUDA Toolkit and cuDNN. Any one drifting broke things.
- **WSL + GCC-12 + dual CUDA 12/13** - GPU passthrough, glibc/GCC compatibility
  for nvcc, and juggling CUDA 12 vs 13 when both were installed.
- **Diminishing returns vs. DirectML** - for the speech models Echophrase runs,
  DirectML on a modern NVIDIA GPU is in the same performance bracket as CUDA,
  ships with Windows, and also works on AMD / Intel / Qualcomm.

### What's in the tree

Nothing CUDA-related. The CUDA plugin downloader, the `cuda` cargo feature, the
CUDA execution-provider code and the CUDA UI were all deleted. Build-affecting
CUDA variables (`CC`, `CXX`, `RUSTFLAGS`, `LIBRARY_PATH`, `NVCC_PREPEND_FLAGS`,
`CMAKE_CUDA_COMPILER`, `CUDA_TOOLKIT_ROOT_DIR`, `ORT_CUDA_VERSION`) and runtime
ones (`CUDA_HOME`, `CUDNN_PATH`) are deliberately absent from `env.yaml.template`
and from `scripts/env/schema.ts`. On WSL, `LD_LIBRARY_PATH` keeps
`/usr/lib/wsl/lib` because the GPU probe reads NVML (VRAM size) from there.

Settings files written by older builds may still contain `execution_provider:
"cuda"`. It is ignored and treated as auto-detect.

GPU acceleration on Linux/WSL is a wanted future feature (see the TODO in
`env.yaml.template`). Linux has no out-of-the-box GPU path like DirectML or
CoreML, so it needs its own design rather than the old CUDA setup. Bringing CUDA
back would be new work, not a flag flip.
