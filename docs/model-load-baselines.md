# Model Load Baselines

Reference wall-clock times for `from_pretrained` model preload, so we can spot
regressions and set user-facing timeout/progress expectations. These measure the
ONNX `from_pretrained` path in `src-tauri/src/transcription/onnx_backend.rs`
(look for the `[ONNX timing]` log lines).

## Measured baselines

| Model | Provider | GPU | Shader cache | Preload time | Source |
|-------|----------|-----|--------------|--------------|--------|
| parakeet-tdt-0.6b-v3 | DirectML | RTX 5080 | warm | ~7.1s (7071ms) | dev run 2026-06-01 |

Captured from the dev log line:

```
[ONNX timing] from_pretrained returned in 7070ms (7.07s)
[ONNX timing] Parakeet TDT v3 ready (total 7071ms)
```

## Warm vs cold (read this before trusting the number)

DirectML JIT-compiles its compute shaders the **first** time a model runs on a
given GPU architecture, then the driver caches the compiled shaders persistently
per machine. So:

- **Warm reload** (shaders already cached): fast - the ~7s above.
- **Cold first run** (new GPU arch, fresh driver/shader cache): can be much
  slower - seconds to minutes. This is the case real users hit on first launch.

Implications:

- The Rust-side preload timeout is **300s** specifically to survive cold JIT on
  new GPU architectures (commit `9ce98cc`).
- A fast preload on a dev box (warm cache) does **not** validate the cold-JIT
  deadlock fix (`590f1b4` moved model load outside the cache mutex; `b384fc0`
  serialized concurrent `from_pretrained` calls). Reproducing that path needs a
  genuinely cold cache - ideally a different/unseen GPU architecture.
- The ORT DirectML runtime is the same version (1.24.2) in dev and in the
  bundled exe, so this is a **cold-vs-warm** distinction, not a dev-vs-exe one.

## How to capture a new baseline

1. Run the app (`bun run tauri:dev`) and watch the `[ONNX timing]` log lines, or
   tail `../logs/Echophrase.log`.
2. Note: model id, execution provider (DirectML/CoreML), GPU, and whether
   the shader cache was warm (a prior run on this machine+GPU) or cold.
3. Add a row to the table above. Keep warm and cold rows distinct - never
   collapse them into one "average."
