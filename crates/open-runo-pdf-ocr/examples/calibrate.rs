#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 試験用: 書体が既知の見本(F:\_gen\mk-cal.ps1 が作る cal.png / cal.tsv)で、明朝体・太字の判定を較正する。
//! 使い方: `cargo run --release -p open-runo-pdf-ocr --example calibrate -- F:\_gen\cal`

use open_runo_pdf_ocr::style::analyze;

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let img = image::open(format!("{dir}/cal.png"))
        .expect("png")
        .to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let rgba = img.into_raw();
    let tsv = std::fs::read_to_string(format!("{dir}/cal.tsv")).expect("tsv");
    let (mut total, mut ok_serif, mut ok_bold) = (0, 0, 0);
    println!("family | style | size | truth(serif,bold,cjk) | contrast | stem_ratio | tightH");
    for row in tsv.lines() {
        let c: Vec<&str> = row.split('\t').collect();
        let (x, y, rw, rh): (i32, i32, i32, i32) = (
            c[3].parse().unwrap(),
            c[4].parse().unwrap(),
            c[5].parse().unwrap(),
            c[6].parse().unwrap(),
        );
        // OCR の行矩形のように、インクの外接矩形に絞る。
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, 0, 0);
        for yy in y.max(0)..(y + rh).min(h as i32) {
            for xx in x.max(0)..(x + rw + 20).min(w as i32) {
                if rgba[(yy as usize * w + xx as usize) * 4] < 128 {
                    x0 = x0.min(xx);
                    y0 = y0.min(yy);
                    x1 = x1.max(xx + 1);
                    y1 = y1.max(yy + 1);
                }
            }
        }
        if x1 <= x0 {
            continue;
        }
        let s = analyze(&rgba, w, h, x0, y0, x1 - x0, y1 - y0).classify(c[8], x1 - x0);
        let t: Vec<&str> = c[7].split(',').collect();
        let (ts, tb) = (t[0] == "1", t[1] == "1");
        total += 1;
        if s.serif == ts {
            ok_serif += 1;
        }
        if s.bold == tb {
            ok_bold += 1;
        }
        if s.serif != ts || s.bold != tb {
            println!(
                "  MISS {} {} {}: truth serif={} bold={}  got serif={} bold={}",
                c[0], c[1], c[2], ts, tb, s.serif, s.bold
            );
        }
        println!(
            "{} | {} | {} | {} | {:.2} | {:.3} | {} | stemem={:.3} | cvbar={:.2} cvstem={:.2}",
            c[0],
            c[1],
            c[2],
            c[7],
            s.contrast,
            s.stem_ratio,
            y1 - y0,
            s.stem_px
                / ((x1 - x0) as f32 / {
                    let nc = c[8].chars().count() as f32;
                    if c[7].ends_with(",1") {
                        nc
                    } else {
                        nc * 0.55
                    }
                }),
            s.cv_bar,
            s.cv_stem
        );
    }
    println!("serif accuracy {ok_serif}/{total}, bold accuracy {ok_bold}/{total}");
}
