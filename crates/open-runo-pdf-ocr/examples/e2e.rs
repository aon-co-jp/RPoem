#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 試験用: PDF に OCR・文字化をかけて保存する。
//! 使い方: `cargo run --release -p open-runo-pdf-ocr --example e2e -- in.pdf out.pdf [サーバーURL] [言語(jpn+eng)] [fast|best]`

use open_runo_pdf_ocr::{process, Quality, Settings};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pdf = std::fs::read(&args[1]).expect("read pdf");
    let mut settings = Settings::default();
    if let Some(url) = args.get(3) {
        settings.server_url = url.clone();
    }
    if let Some(l) = args.get(4) {
        settings.languages = l.split('+').map(|s| s.to_string()).collect();
    }
    if args.get(5).map(|s| s.as_str()) == Some("fast") {
        settings.quality = Quality::Fast;
    }
    if let Some(b) = args.get(6).and_then(|s| s.parse().ok()) {
        settings.batch_pages = b;
    }
    if let Some(url) = args.get(7) {
        settings.fill = true;
        settings.fill_server_url = url.clone();
    }
    let cache = std::env::temp_dir().join("pdf-r2l-e2e-cache");
    let t = std::time::Instant::now();
    let mut last = String::new();
    let result = process(
        &pdf,
        &settings,
        &cache,
        &mut |p| {
            let s = format!("{:?} {}/{}", p.stage, p.done, p.total);
            if s != last {
                eprintln!("[{:.1}s] {s}", t.elapsed().as_secs_f32());
                last = s;
            }
        },
        &|| false,
    );
    match result {
        Ok((out, report)) => {
            std::fs::write(&args[2], out).expect("write");
            eprintln!("done in {:.1}s: {report:?}", t.elapsed().as_secs_f32());
        },
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        },
    }
}
