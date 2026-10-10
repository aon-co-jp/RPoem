#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 試験用: 画像を高解像度化する。使い方: `sr_cli in.jpg out.png <fhd|qhd|4k|5k|8k|16k> [ai|plain]`
use open_runo_sr::{upscale, Format, Options, Target};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let target = match a[3].as_str() {
        "fhd" => Target::FullHd,
        "qhd" => Target::Qhd,
        "4k" => Target::Uhd4k,
        "5k" => Target::Uhd5k,
        "8k" => Target::Uhd8k,
        _ => Target::Uhd16k,
    };
    let ai = a.get(4).map(|s| s.as_str()) != Some("plain");
    let format = if a[2].ends_with(".jpg") { Format::Jpeg } else { Format::Png };
    let input = std::fs::read(&a[1]).expect("read");
    let t = std::time::Instant::now();
    let mut last = -1i32;
    let out = upscale(&input, Options { target, ai, format, threads: 0 }, &mut |p| {
        let pct = (p.fraction * 100.0) as i32;
        if pct / 10 != last / 10 {
            eprintln!("[{:.1}s] {pct}% {}", t.elapsed().as_secs_f32(), p.stage);
            last = pct;
        }
    }, &|| false)
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&a[2], &out.bytes).expect("write");
    eprintln!("{}x{} ai={} {:.1}s {} KB kernel={}", out.width, out.height, out.used_ai, t.elapsed().as_secs_f32(), out.bytes.len() / 1024, open_runo_sr::kernel_name());
}
