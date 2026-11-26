# cool_fox

### D3D12 Compatibility & Performance Engineering

**An independent experimental engineering fork focused on graphics compatibility, CPU/GPU coordination, memory management and performance research.**

Maintained by **[@larpingtung](https://github.com/larpingtung)**.

cool_fox is a Rust-based research project built around a Windows Direct3D 12 compatibility layer. Its focus is the engineering underneath graphics workloads: command processing, resource lifetime, synchronization, memory movement, pipeline preparation and repeatable diagnostics.

The aim is straightforward: find measurable bottlenecks, fix correctness problems, reduce avoidable overhead and make performance work reproducible.

> **Status:** Experimental work in progress. This repository is not a production release and makes no claim of verified application-level performance gains.

---

## Development system

The primary local development and test machine:

| Component | Configuration |
|---|---|
| CPU | AMD Ryzen 5 7500F |
| GPU | NVIDIA GeForce RTX 4060 |
| Dedicated VRAM | 8 GB GDDR6 |
| System memory | 32 GB DDR5 |
| Operating system | Windows 10 x64 |
| Graphics API | Direct3D 12 |
| Main implementation language | Rust |

These are the maintainer's development-system specifications, not minimum requirements or a compatibility guarantee.

## Engineering priorities

### CPU/GPU coordination
- Investigate fence handling, queue dependencies and unnecessary waits.
- Identify CPU submission bottlenecks and GPU idle gaps.
- Improve command recording, batching and submission preparation.
- Preserve correct ordering and resource-lifetime guarantees.

### Memory and resource lifetime
- Audit allocation ownership, cleanup paths and unbounded retention.
- Investigate upload, readback and staging-buffer reuse.
- Track deferred resource retirement and descriptor reuse.
- Distinguish confirmed leaks from caches, allocator reservations and temporary peaks.

### CPU performance and concurrency
- Profile worker scheduling, lock contention and single-thread bottlenecks.
- Reduce unnecessary copies, allocations and repeated work.
- Evaluate data locality, SIMD and safe parallel execution.
- Measure changes rather than assuming more threads are faster.

### GPU resource and pipeline management
- Investigate resource residency, heap and descriptor management.
- Reduce avoidable resource transitions and submission overhead.
- Examine pipeline initialization and warm-up behavior.
- Keep hardware-specific paths guarded by correctness checks and portable fallbacks.

### Streaming and diagnostics
- Audit file-handle and resource ownership.
- Measure decompression, transfer and temporary-buffer costs.
- Use synthetic workloads and frame replay where suitable captures are available.
- Record enough timing and allocation data to make regressions visible.

## Current research direction

The current development effort is focused on several concrete areas:

- Upload-memory ownership and address-range tracking.
- Correct handling of GPU fence failures and device-removal conditions.
- Resource retirement only after confirmed GPU completion.
- Reducing avoidable waits and background warm-up overhead.
- DirectStorage bookkeeping and file-handle lifecycle.
- Understanding memory high-water marks and the lifetime of transient buffers.
- Measuring CPU submission work separately from GPU execution.

Some experiments and fixes are being evaluated in a local working copy. **A local change should not be considered part of this public repository until it appears in its published commit history.** Reported synthetic benchmark results are not equivalent to application-level FPS gains.

## Build and diagnostics

The project uses a pinned Rust toolchain and the Windows MSVC build tools/Windows SDK.

A release build can be produced from the repository root with:

```powershell
cargo build --release --locked -p victory
```

For the diagnostics profile:

```powershell
cargo build --locked --profile diag -p victory --features diagnostics
```

See [docs/replay.md](docs/replay.md) for the existing frame-recording and replay workflow, and [docs/findings.md](docs/findings.md) for inherited performance notes. A replay is only representative of the workload captured; it does not replace full runtime testing.

## Validation status and limitations

Validation work may include source review, Rust compilation, unit/integration tests, static analysis, isolated microbenchmarks and synthetic graphics replay.

**The latest experimental optimization changes described here have not yet been tested against a complete target-application build.** No claims are made about real-world FPS gains, sustained runtime performance, minimum RAM/VRAM requirements, long-session stability or broad hardware compatibility.

Any published measurement should identify:
- The exact source revision and build configuration.
- The hardware and driver used.
- The workload and test method.
- Whether it is a microbenchmark, synthetic replay or live runtime measurement.
- Baseline and modified results, including regressions and limitations.

Correctness and reproducibility come before performance claims.

## Project lineage and credits

This repository is an independently maintained derivative of [VIctory](https://github.com/Marvisak/VIctory) and related open-source work. It builds on contributions from the original project and other developers; this fork does not claim authorship of the inherited implementation.

Credits from the inherited project and codebase:

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its contributors: loader, runtime services, Direct3D 12 backend and shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork this codebase started from and its contributors.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: rendering fixes, cube gradients and sparse-texture work.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**: ambient-light ideas, pipeline workers and work by romainhedouin.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: memory-view guard work and related contributions, including work by Spexivus, joroyt, tightfall and Joshua Odom.
- **Geckomacabre** and **cxy0072019-sketch**: texture read-back contributions.
- **[XCL](https://github.com/ivall/xcl)** was consulted as a reference for compatibility behavior; its code is not included here.
- The inherited fork's changes included work prepared with [Claude Code](https://claude.com/claude-code).

Original notices and contributor attribution in the source and Git history are retained.

## License

GPL-3.0-or-later, as inherited from VIctory. See [LICENSE](LICENSE). The asset-policy exception is documented in [docs/asset-policy.md](docs/asset-policy.md).
