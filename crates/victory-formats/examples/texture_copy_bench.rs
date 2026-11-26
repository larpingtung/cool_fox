//! Run with `cargo run --release -p victory-formats --example texture_copy_bench`.

use std::{hint::black_box, time::Instant};
use victory_formats::texture::{Desc, Dimension, Layout, Swizzle};

fn median_us(reps: u32, mut copy: impl FnMut()) -> f64 {
    copy();
    let mut samples = [0.0; 7];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..reps {
            copy();
        }
        *sample = start.elapsed().as_secs_f64() * 1e6 / f64::from(reps);
    }
    samples.sort_by(f64::total_cmp);
    samples[3]
}

fn main() {
    for (label, swizzle, n, width, height, depth, mip) in [
        ("R8 D", Swizzle::SW_64KB_D, 1, 513, 511, 1, 0),
        ("RGBA8 D", Swizzle::SW_64KB_D, 4, 513, 511, 1, 0),
        ("BC7 D", Swizzle::SW_64KB_D, 16, 512, 512, 1, 0),
        ("R16 volume S", Swizzle::SW_64KB_S, 2, 127, 129, 15, 0),
        ("RGBA8 volume S", Swizzle::SW_64KB_S, 4, 127, 129, 15, 0),
        ("R8 D tail", Swizzle::SW_64KB_D, 1, 513, 511, 1, 3),
        ("RGBA8 linear", Swizzle::LINEAR, 4, 513, 511, 1, 0),
    ] {
        let layout = Layout::new(&Desc {
            dimension: if depth == 1 {
                Dimension::Tex2d
            } else {
                Dimension::Tex3d
            },
            width,
            height,
            depth,
            mips: 4,
            element_bytes: n,
            block: if n == 16 { 4 } else { 1 },
            samples: 1,
            swizzle,
        })
        .unwrap();
        let m = &layout.mips[mip as usize];
        let bytes = (m.width * n) as usize;
        let pitch = bytes + 13;
        let plane = pitch * m.height as usize;
        let input: Vec<_> = (0..plane * m.depth as usize)
            .map(|i| (i.wrapping_mul(0x9e37_79b9) >> 24) as u8)
            .collect();
        let mut tiled = vec![0; layout.size as usize];
        let mut output = vec![0; input.len()];
        let reps = if mip == 0 { 48 } else { 512 };
        let tile = median_us(reps, || {
            layout
                .tile(
                    black_box(&mut tiled),
                    mip,
                    0,
                    black_box(&input),
                    pitch,
                    plane,
                )
                .unwrap();
        });
        let detile = median_us(reps, || {
            layout
                .detile(
                    black_box(&tiled),
                    mip,
                    0,
                    black_box(&mut output),
                    pitch,
                    plane,
                )
                .unwrap();
        });
        for (actual, expected) in output.chunks_exact(pitch).zip(input.chunks_exact(pitch)) {
            assert_eq!(actual[..bytes], expected[..bytes]);
            assert!(actual[bytes..].iter().all(|&b| b == 0));
        }
        println!("{label:16} {tile:8.2} us tile {detile:8.2} us detile");
    }
}
