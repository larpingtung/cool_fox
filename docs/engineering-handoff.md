# Engineering handoff

## DirectStorage worker-count experiment

- Base: merged `main` at `5f13590` (PR #8). Hypothesis: the DirectStorage pool can compete for CPU time with shader translation; the independent pipeline pool ordinarily sizes to two-thirds of logical processors on this machine (eight), before optional dedicated workers. `crates/victory-hle/src/modules/dstorage.rs` keeps the default `available_parallelism()` (fallback four). `VICTORY_DSTORAGE_WORKERS` is read once on first use: a valid integer is clamped to `1..=min(available_parallelism, 64)`; an unparsable value warns and uses the default. Startup logs selected and successfully started counts and each failed spawn. If no worker can start, queue creation fails rather than accepting requests that cannot run.
- Native Windows 11 x64 (22631), Ryzen 5 7500F (6 cores/12 logical), RTX 4060 (driver 621.40), NVMe SSD, Rust 1.98.1 MSVC. Ran five randomized blocks (Python `random.Random(42)`) of all combinations of 0/8 concurrent shader translation threads and unset/1/2/4/6/8/12 storage workers: **70 separate release test processes**. Unset started 12 in every run. The eight normal-priority threads repeatedly translate one synthetic compute shader through the runtime translator, without its disk cache. They are not the real priority-adjusted pipeline pool and do not create PSOs.
- The ignored `scheduler_worker_sweep` enqueues and submits to the actual scheduler and checks ordered status, output bytes and both physical-page layouts. Each case is 16 measured bursts of 32 requests after two warm-up bursts. File reads and zlib output are 512 KiB per request; BCPack is the existing two-block BC4 fixture (16 output bytes), so its throughput reflects tiny requests and is not representative of bulk BCPack decoding. The file is warm in the OS cache. The timer excludes setup and enqueue, includes submit and waiting for the batch status; request times are from submit to each worker's recorded completion.

Reproduce on Windows (PowerShell; run each count in a new test process):

```powershell
foreach ($shaders in 0,8) {
    $env:VICTORY_DSTORAGE_BENCH_SHADERS = "$shaders"
    foreach ($workers in 'auto',1,2,4,6,8,12) {
        if ($workers -eq 'auto') { Remove-Item Env:VICTORY_DSTORAGE_WORKERS -ErrorAction SilentlyContinue }
        else { $env:VICTORY_DSTORAGE_WORKERS = "$workers" }
        cargo test --release --locked -p victory-hle --lib modules::dstorage::tests::worker_sweep::scheduler_worker_sweep -- --exact --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'DirectStorage benchmark failed' }
    }
}
Remove-Item Env:VICTORY_DSTORAGE_WORKERS,Env:VICTORY_DSTORAGE_BENCH_SHADERS
```

Use default streaming/staging settings, and leave per-request tracing off. Repeat and vary process order. Each cell below is the median of the five process results: **MiB/s / p50 µs / p95 µs** (percentiles computed per process, not pooled).

| Shader threads | Storage workers | Raw | Zlib | Zlib contiguous pages | Zlib scattered pages | BCPack |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | unset (12) | 114352/93/150 | 73498/137/217 | 67475/139/223 | 13527/629/1183 | 7.3/44/67 |
| 0 | 1 | 26676/303/680 | 13824/595/1111 | 12297/670/1322 | 8750/939/1826 | 5.6/48/86 |
| 0 | 2 | 50812/169/343 | 25010/334/629 | 23756/350/663 | 16694/503/951 | 8.7/32/57 |
| 0 | 4 | 100278/87/155 | 49322/180/334 | 46247/184/352 | 18988/470/867 | 12.7/24/37 |
| 0 | 6 | 112518/83/138 | 57121/152/271 | 61699/148/257 | 15117/549/1085 | 11.0/29/43 |
| 0 | 8 | 113546/86/133 | 64723/144/240 | 61866/152/248 | 13668/621/1189 | 8.6/39/67 |
| 0 | 12 | 112291/92/151 | 67942/143/231 | 65127/143/235 | 12467/670/1308 | 7.3/42/60 |
| 8 | unset (12) | 46165/184/414 | 25402/303/700 | 50925/195/314 | 11936/700/1243 | 6.3/41/90 |
| 8 | 1 | 25940/315/619 | 10120/795/1569 | 9529/867/1619 | 6624/1222/2443 | 3.4/76/134 |
| 8 | 2 | 25296/283/982 | 18219/465/851 | 17836/479/872 | 11927/692/1312 | 5.5/47/84 |
| 8 | 4 | 38594/188/496 | 26780/315/621 | 28605/310/574 | 11925/733/1437 | 5.5/46/109 |
| 8 | 6 | 58920/152/295 | 29794/302/605 | 44233/198/361 | 13850/618/1196 | 6.0/42/91 |
| 8 | 8 | 34617/191/536 | 29472/280/625 | 53615/179/291 | 12121/685/1304 | 5.1/52/89 |
| 8 | 12 | 59690/149/270 | 32151/273/509 | 52280/180/305 | 11453/608/1285 | 6.1/43/78 |

- Peak queued depth was 32 reads, with no staging waits at the 256 MiB default staging floor; the largest reservation was 6,375,936 bytes. With eight shader threads, scattered zlib's summed busy-worker time over 512 requests was 70.7 ms at 6 workers and 84.2 ms at 12 (98.0 ms at unset). Busy time is summed wall time in the worker, not on-core CPU time. Per-process output also reports shader translations completed during measured bursts.
- Smaller pools help some synthetic cases but hurt others. For example, with eight shader threads, 6 vs explicit 12 improved median scattered zlib throughput but lowered contiguous zlib throughput. The unset and explicit 12 controls also varied despite selecting identical counts. The test uses a single lane, short warm-cache bursts and synthetic CPU contention. It does not establish whether 12 workers are excessive in a title. **Keep the default; no frame-rate or gameplay claim.**
- Windows validation (locked, using cached dependencies and DXIL validator 1.8.2505.32): `cargo test --locked -p victory-hle --lib modules::dstorage::tests` passed (22 passed, 2 ignored); all 70 ignored release benchmark invocations passed. Both `cargo test --workspace --locked` and `cargo test --workspace --locked -- --test-threads=1` failed at `list::tests::repeated_lists_run_through_the_title_queue` (`1` vs `2` at `list.rs:2621`); the same assertion failed when run on clean `5f13590`. The supplemental serial `--no-fail-fast` run reached `victory-hle` (155 passed, 3 ignored). `cargo clippy --workspace --all-targets --locked -- -D warnings` failed on existing `victory-gpu` diagnostics, also reproduced on clean `5f13590`. No tests or lints were disabled for this change.

## Phase 4.1 — Stream the known-pipelines book

- Base: merged `main` at `65a192b651477773d41ea36c859c26ced6587286`. The old scanner read all of `known-pipelines.bin` into a `Vec` on each scan. `crates/victory-gpu/src/pipeline/known.rs` now uses a 64 KiB `BufReader` and one reusable record buffer. File format, order, numbering, and defaults are unchanged. Incomplete tails stop at the last complete record; read failures do not authorize truncation. The length check is best-effort, not cross-process locking.
- Scanner benchmark: Windows 11 x64, Ryzen 5 7500F, Rust 1.98.1 MSVC, `rustc -O`. Compiled each revision's exact scanner; scanned the same warm-cache synthetic 41.34 MiB book (84,000 records with 512-byte payloads and a torn final record). One warm-up per variant; ten alternating fresh-process runs per variant, checking record count, order, and final offset. Median scan: **8.58 → 5.84 ms**. Median peak process working set: **44.50 → 3.24 MiB** (Windows `PeakWorkingSetSize`). This is scanner-only, not cold-start, gameplay, or GPU telemetry. Scanner memory still scales with the largest record; caller-owned keys or selected records are not included.
- Native release workspace: `cargo test --locked --release --workspace --no-fail-fast -- --test-threads=1`, with DXIL validator 1.8.2505.32: **940 passed, 0 failed, 45 ignored**. Scanner coverage includes missing books, record order, empty and larger-than-buffer records, torn headers/payloads, and an unreadable-as-file path. Parallel GPU testing hit an intermittent queue assertion (`list.rs:2621`, expected 2, got 1) in `behind` mode; the same assertion failed in `thin` mode in two of 32 parallel GPU library runs on unchanged `main`. Both variants also had intermittent watch failures. No tests were disabled.
- `cargo clippy --locked --workspace --all-targets` passed with warnings. With `-D warnings`, both revisions failed on the same existing texture-test precedence and loader-test transmute annotation lints.
- Next: benchmark card-buffer thresholds on the RTX 4060 before changing any default.

## Phase 4 — RTX 4060 baseline (no GPU defaults changed)

- On merged `main` (`c7e3275`), `card_compare_costs` measured 0.49, 0.53, 1.97 and 49.69 ms for 80 rounds of five passes over 1, 5, 64 and 500 64-KiB pieces respectively (best of six, includes submit/wait). One pass over 4,000 pieces took 1.04 ms. This measures the compare pass, not the cost or benefit of placing title buffers in VRAM; keep the 16/512 MiB limits for adapters with budgets up to 12 GiB until a title workload can sweep the actual thresholds.
- `texture_checks_per_submission` confirmed existing hash deduplication (576 checks vs 48, 383 vs 32 ms in its synthetic CPU-only workload). It does not profile descriptor locks or driver copies. No descriptor-sync change without those measurements.
- `bench_indirect_draw_barriers` (RTX 4060, driver 621.40; 3,200 one-record draws, median of nine frames) measured current one-at-a-time vs windows of 32 at 21.24 vs 0.91 ms GPU and 4.13 vs 0.44 ms CPU recording. The benchmark records first, then submits and waits; it does not measure CPU/GPU overlap. These are existing paths, not a new speedup. No representative frame capture was available; do not infer that passes can be removed or budget behavior changed. Use `docs/replay.md` for GPU pass attribution and live queue timestamps for overlap.

## Phase 4 — Specialize four-byte texture copies

- `crates/victory-formats/src/texture.rs` uses the existing fixed-size copy path for aligned four-byte runs. Irregular, unaligned and final runs keep the byte-copy fallback. Layouts, validation and public API are unchanged.
- `cargo run --locked --release -p victory-formats --example texture_copy_bench`: Windows 11 x64, Ryzen 5 7500F, Rust 1.98.1 MSVC; identical synthetic inputs and executable build settings, one warm-up per variant, 12 alternating runs per variant. Each run reports the median of seven batches. For a 127 × 129 × 15 R16 volume, median tile/detile latency went from **300.0/299.0 µs to 107.5/91.2 µs**. Other sampled shapes ranged from 6.3% faster to 7.5% slower; the latter was R8 tail tiling (0.730 → 0.785 µs). The R8 base mip took 26.64 → 27.73 µs to tile. These are hot-buffer CPU copy timings, not upload or frame times. A two-byte specialization alone did not reproduce the gain and was dropped.
- Native release tests for `victory-formats`, `victory-gpu` and `victory-hle`, serial with DXIL validator 1.8.2505.32: **540 passed, 0 failed, 41 ignored**. Added an odd-sized R16 volume to the existing per-element tiled address, round-trip and padding check.
