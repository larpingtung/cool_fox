use super::*;
use crate::modules::xmem;

/// Small synthetic wave32 compute program in the ACCC/FSPX container format.
fn shader() -> Vec<u8> {
    let words = [
        u32::from_le_bytes(*b"DXBC"), 0, 0, 0, 0, 1, 192, 1, 36,
        u32::from_le_bytes(*b"ACCC"), 148,
        u32::from_le_bytes(*b"FSPX"), 1, 3, 148,
        0, 64, 28, 0, 6, 92, 32, 0, 7, 124, 24, 0,
        0, 0, 0, 0x0001_0020, 0x0010_0001, 0, 0,
        0xBE82_0381, 0x8102_8102, 0x7E00_0202, 0x7E02_0200,
        0x7E04_0201, 0x7E06_0202, 0x7E08_0203, 0xBF81_0000,
        0x403, 4, 8, 0, 0, 0x40,
    ];
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

#[test]
#[ignore = "run alone in a fresh release test process per worker count"]
fn scheduler_worker_sweep() {
    let shader_threads = std::env::var("VICTORY_DSTORAGE_BENCH_SHADERS")
        .map_or(0, |v| v.parse::<usize>().expect("shader worker count"));
    assert!(shader_threads <= 64);
    let shader = shader();
    let compile = || victory_gpu::shader_cache::precompile(&shader).expect("synthetic shader must translate");
    if shader_threads > 0 {
        compile(); // initialize the host outside the measurement
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    let translated = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        struct Stop<'a>(&'a std::sync::atomic::AtomicBool);
        impl Drop for Stop<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
            }
        }
        let _stop = Stop(&stop);
        for _ in 0..shader_threads {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    std::hint::black_box(compile());
                    translated.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
        sweep(shader_threads, &translated);
    });
}

fn sweep(shader_threads: usize, translated: &std::sync::atomic::AtomicUsize) {
    const BATCH: usize = 32;
    const PAGES: usize = 8;
    const ROUNDS: usize = 18;
    let scheduler = scheduler().unwrap();
    let workers = *STARTED_WORKERS.get().unwrap();
    assert!(scheduler.lock().lanes.is_empty(), "run this test by itself");
    assert_eq!(set_staging_buffer_size(factory().cast(), 32 << 20), S_OK);
    let staging = scheduler.lock().staging;
    with_test_tables();
    let plain = pattern(PAGES * PAGE);
    let packed = compress_zlib(&plain);
    let bc4 = bc4_blocks();
    let (_dir, file) = file_with(&[plain.as_slice(), &packed, &BC4_STREAM].concat());
    let queue = new_queue(0);
    let array = status_array(1);
    let mut buffers = vec![vec![0u8; plain.len()]; BATCH];
    let mut pages = vec![0; BATCH * PAGES];
    let mut n = pages.len();
    assert_eq!(xmem::XMemAllocatePhysicalPages(0, &mut n, pages.as_mut_ptr()), 1);
    assert_eq!(n, pages.len());
    let mut scattered = pages.clone();
    for group in scattered.chunks_exact_mut(PAGES) {
        group.reverse(); // no adjacent pages in request order
    }
    assert!(xmem::page_window(&pages[..PAGES], 0, plain.len()).is_some());
    assert!(xmem::page_window(&scattered[..PAGES], 0, plain.len()).is_none());

    for (case, options, offset, source, expected) in [
        ("raw", 0, 0, plain.len(), plain.as_slice()),
        ("zlib", ZLIB, plain.len(), packed.len(), plain.as_slice()),
        ("zlib-pages", ZLIB | PAGE_DESTINATION, plain.len(), packed.len(), plain.as_slice()),
        ("zlib-scatter", ZLIB | PAGE_DESTINATION, plain.len(), packed.len(), plain.as_slice()),
        ("bcpack", 4 << 1, plain.len() + packed.len(), BC4_STREAM.len(), bc4.as_slice()),
    ] {
        let mut samples = Vec::with_capacity((ROUNDS - 2) * BATCH);
        let (mut bytes, mut elapsed, mut busy, mut waits, mut peak) = (0, Duration::ZERO, Duration::ZERO, 0, 0);
        let lists = if case == "zlib-scatter" { &mut scattered } else { &mut pages };
        let mut compiles = 0;
        for round in 0..ROUNDS {
            for (buffer, list) in buffers.iter_mut().zip(lists.chunks_exact_mut(PAGES)) {
                let destination = if options & PAGE_DESTINATION != 0 {
                    list.as_mut_ptr().cast()
                } else {
                    buffer.as_mut_ptr().cast()
                };
                enqueue(queue, &request(options, destination, expected.len(), file, offset as u64, source));
            }
            enqueue_status(queue, array, 0);
            let compiled_before = translated.load(Ordering::Relaxed);
            {
                let mut lanes = scheduler.lock();
                lanes.done = Default::default();
                lanes.refused = 0;
                lanes.staging_peak = 0;
                lanes.since = None;
                lanes.bench = Some((Instant::now(), Vec::with_capacity(BATCH)));
            }
            void(queue, 0x30);
            let (mut lanes, _) = scheduler.finished.wait_timeout_while(scheduler.lock(), Duration::from_secs(10), |_| {
                result(array, 0) == E_PENDING
            }).unwrap();
            let status = result(array, 0);
            if status == E_PENDING {
                drop(lanes);
                void(queue, 0x40); // drain running writes before any destination can drop
                panic!("{case}: batch did not complete");
            }
            assert_eq!(status, S_OK, "{case}");
            let (submitted, latencies) = lanes.bench.take().unwrap();
            assert_eq!(latencies.len(), BATCH);
            assert_eq!(lanes.done.0, BATCH as u64);
            if round >= 2 {
                elapsed += submitted.elapsed();
                compiles += translated.load(Ordering::Relaxed) - compiled_before;
                samples.extend(latencies);
                bytes += lanes.done.1;
                busy += lanes.done.2;
                waits += lanes.refused;
                peak = peak.max(lanes.staging_peak);
            }
            drop(lanes);
            if round == 0 || round == ROUNDS - 1 {
                if options & PAGE_DESTINATION != 0 {
                    for list in lists.chunks_exact(PAGES) {
                        for (page, data) in list.iter().zip(expected.chunks(PAGE)) {
                            let mut window = xmem::page_window(std::slice::from_ref(page), 0, data.len()).unwrap();
                            assert_eq!(window.bytes(), data);
                            window.bytes().fill(0);
                        }
                    }
                } else {
                    for buffer in &mut buffers {
                        assert_eq!(&buffer[..expected.len()], expected);
                        buffer.fill(0);
                    }
                }
            }
        }
        samples.sort_unstable();
        let micros = |p: usize| samples[(samples.len() * p).div_ceil(100) - 1].as_secs_f64() * 1e6;
        // Submit inserts all BATCH reads under one lock before a worker can pop one.
        println!("dstorage-bench case={case} workers={workers} shaders={shader_threads} requests={} mib_s={:.2} p50_us={:.2} p95_us={:.2} peak_depth={BATCH} busy_ms={:.3} staging_waits={waits} peak_staging_bytes={peak} staging_limit_bytes={staging} shader_translations={compiles}",
            samples.len(), bytes as f64 / (1 << 20) as f64 / elapsed.as_secs_f64(), micros(50), micros(95), busy.as_secs_f64() * 1e3);
    }
    void(queue, 0x40);
    assert_eq!(xmem::XMemFreePhysicalPages(n, pages.as_ptr()), 1);
    for object in [queue, array, file] {
        let release: extern "system" fn(*mut c_void) -> u32 = call(object, 16);
        assert_eq!(release(object), 0);
    }
}
