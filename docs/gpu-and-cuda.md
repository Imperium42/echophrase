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

### What's still in the tree

Some backend modules (e.g. `candle_whisper.rs`, `parakeet.rs`) still have
CUDA `cfg` paths, and `ort_plugin/downloader.rs` can still fetch a CUDA-enabled
ONNX Runtime prebuilt. None of it is wired into release builds. Don't build new
features on it.

Build-affecting CUDA variables (`CC`, `CXX`, `RUSTFLAGS`, `LIBRARY_PATH`,
`NVCC_PREPEND_FLAGS`, `CMAKE_CUDA_COMPILER`, `CUDA_TOOLKIT_ROOT_DIR`,
`ORT_CUDA_VERSION`) and runtime ones (`CUDA_HOME`, `CUDNN_PATH`) are deliberately
absent from `env.yaml.template` and from `scripts/env/schema.ts`. On WSL,
`LD_LIBRARY_PATH` keeps `/usr/lib/wsl/lib` because the GPU probe reads NVML from
there.

GPU acceleration on Linux/WSL is a wanted future feature (see the TODO in
`env.yaml.template`). Linux has no out-of-the-box GPU path like DirectML or
CoreML, so it needs its own design rather than the old CUDA setup.

### Re-enabling CUDA (unsupported, at your own risk)

The env schema objects are `.passthrough()`, so:

1. Re-add the build vars to the `wsl:` / `linux:` blocks of your `env.yaml`.
2. Optionally re-declare them in `scripts/env/schema.ts` to validate them.
3. Uncomment the CUDA exports in `scripts/wsl/dev.sh`.
4. Build with `--features cuda`.

Host requirements: CUDA Toolkit 13.0+, cuDNN 9.19+, driver 525+, GCC-12.
Blackwell (RTX 50-series) needs CUDA 13+.
