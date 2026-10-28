#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 試験用: かすれ文書(`mk_faded.py` で作った画像、文字の位置が既知)で、字形の照合が正しい字を上位に選べるかを測る。
//! 使い方: `cargo run --release -p open-runo-pdf-ocr --example vis_probe -- faded.pdf truth.json font.ttf [x0 line_h char_w y0 dpi_scale]`
use open_runo_glyph::{GlyphIndex, Glyphs};
use open_runo_pdf_ocr::{enhance, render, visual};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let pdf = std::fs::read(&a[1]).unwrap();
    let truth: Vec<String> = sonic_rs::from_slice(&std::fs::read(&a[2]).unwrap()).unwrap();
    // 画像上の配置(mk_faded.py と同じ): 左端 30px、行の間隔 40px、文字の幅 19px、1 行目の上端 30px。
    let (x0, lh, cwid, y0) = (30.0f32, 40.0f32, 19.0f32, 30.0f32);
    let r = render::render_pages(&pdf, 0..1, 3000).unwrap().remove(0);
    let page = enhance::normalize_contrast(&r).unwrap_or(r.clone());
    let scale = page.width as f32 / 1000.0;
    let fonts: Vec<Vec<u8>> = a[3].split(',').map(|p| std::fs::read(p).unwrap()).collect();
    let g = Glyphs::with_fonts(&fonts.iter().map(|v| &v[..]).collect::<Vec<_>>()).unwrap();
    // 候補: 全ての行の全ての文字(重複なし) = およそ 100 種類の漢字・かな。
    let mut pool: Vec<char> = Vec::new();
    for l in &truth {
        for c in l.chars() {
            if !pool.contains(&c) {
                pool.push(c);
            }
        }
    }
    let index = GlyphIndex::build(&g, &pool);
    // 位置ずれの大きさ(1 文字の幅に対する割合、例 0.3)。
    let jit: f32 = std::env::var("JITTER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let (mut top1, mut top3, mut total, mut evsum) = (0, 0, 0, 0.0f32);
    let mut k = 0u32;
    for (li, line) in truth.iter().enumerate() {
        for (ci, ch) in line.chars().enumerate() {
            if ch.is_ascii() || !ch.is_alphanumeric() {
                continue;
            }
            k += 1;
            let (jx, jy) = (
                jit * (((k * 7) % 5) as f32 - 2.0) / 2.0,
                jit * (((k * 3) % 5) as f32 - 2.0) / 2.0,
            );
            let cx = (x0 + (ci as f32 + 0.5) * cwid) * scale;
            let cy = (y0 + li as f32 * lh + cwid * 0.62) * scale;
            let Some(cell) = visual::cell(
                &page,
                cx + jx * cwid * scale,
                cy + jy * cwid * scale,
                cwid * scale * 2.0,
            ) else {
                continue;
            };
            let (scored, _) = index.rank_cpu(&cell.map, 200, &[]);
            let rank = scored.iter().position(|(_, c)| *c == ch).unwrap_or(999);
            total += 1;
            evsum += cell.evidence;
            if rank == 0 {
                top1 += 1;
            }
            if rank < 3 {
                top3 += 1;
            }
        }
    }
    println!(
        "chars={total} candidates={} top1={:.0}% top3={:.0}% mean evidence={:.2}",
        pool.len(),
        100.0 * top1 as f32 / total as f32,
        100.0 * top3 as f32 / total as f32,
        evsum / total as f32
    );
}
