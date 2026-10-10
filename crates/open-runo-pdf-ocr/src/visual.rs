//! 字形による照合のための、ページ側の処理: かすれた文字の **升目**(ページ画像の切り抜き)を作る。
//!
//! 言語モデルは「文脈にふさわしい字」を挙げるが、ページ上の実際のインクを見ていない。ここでは、疑わしい文字の升目を切り出して
//! 濃淡の格子にし、インクがあるという証拠の強さ `evidence` を求める(まるごと消えた升目は `evidence` が小さく、視覚の重みは自動的に下がる)。
//! 升目と候補の字形の照合(升目の中で字形をずらした正規化相互相関)は、共有クレート `open-runo-glyph` が行う
//! (アプリ内の CPU 参照実装 または サーバー `aruaru-llm` の `POST /v1/ocr/glyph-rank` = `open-cuda` の `sgemm`)。

use crate::render::PageRaster;
pub use open_runo_glyph::{G, N};

/// 升目の濃淡(0 = 白、1 = 濃いインク)と、インクがあるという証拠の強さ(0〜1)。
#[derive(Debug, Clone)]
pub struct Cell {
    pub map: Vec<f32>,
    pub evidence: f32,
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f32 * p).round() as usize]
}

/// ページの画像から、中心 `(cx, cy)`、一辺 `side`(画素)の升目を切り出して格子にする。
pub fn cell(raster: &PageRaster, cx: f32, cy: f32, side: f32) -> Option<Cell> {
    if side < 6.0 {
        return None;
    }
    let (x0, y0) = (cx - side / 2.0, cy - side / 2.0);
    let step = side / N as f32;
    let mut dark = vec![0f32; N * N];
    for gy in 0..N {
        for gx in 0..N {
            // 格子の 1 マスにあたる範囲を 2×2 で標本化して平均する。
            let (mut s, mut c) = (0.0, 0.0);
            for sy in 0..2 {
                for sx in 0..2 {
                    let px = (x0 + (gx as f32 + 0.25 + 0.5 * sx as f32) * step).floor() as i32;
                    let py = (y0 + (gy as f32 + 0.25 + 0.5 * sy as f32) * step).floor() as i32;
                    if px >= 0 && py >= 0 && (px as usize) < raster.width && (py as usize) < raster.height {
                        let i = (py as usize * raster.width + px as usize) * 4;
                        let l = (299 * raster.rgba[i] as u32 + 587 * raster.rgba[i + 1] as u32 + 114 * raster.rgba[i + 2] as u32) as f32 / 1000.0;
                        s += 1.0 - l / 255.0;
                        c += 1.0;
                    }
                }
            }
            dark[gy * N + gx] = if c > 0.0 { s / c } else { 0.0 };
        }
    }
    let mut sorted = dark.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    // 背景 = 中央値、インク = 最も濃い 2% の平均(升目を広めに取るため、字の画素が占める割合は小さい)。
    let lo = percentile(&sorted, 0.5);
    let top = &sorted[sorted.len() - (sorted.len() / 50).max(1)..];
    let hi = top.iter().sum::<f32>() / top.len() as f32;
    let span = hi - lo;
    // 証拠: 升目の中に、背景よりはっきり濃い部分があるか(ざらつきだけの升目は、ここが小さい)。
    let evidence = ((span - 0.3) / 0.3).clamp(0.0, 1.0);
    let map: Vec<f32> = dark.iter().map(|d| ((d - lo) / span.max(1e-3)).clamp(0.0, 1.0)).collect();
    Some(Cell { map: open_runo_glyph::blur3(&map, N), evidence })
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_runo_glyph::{GlyphIndex, Glyphs};

    fn font() -> Option<Glyphs> {
        // 試験用: 手元にあるフォントがあれば使う(無ければ、この試験は何もしない)。
        for p in ["F:/tessdata-jpn/fonts/NotoSansJP-VF.ttf", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"] {
            if let Ok(b) = std::fs::read(p) {
                if let Some(g) = Glyphs::with_fonts(&[&b[..]]) {
                    return Some(g);
                }
            }
        }
        None
    }

    /// 字形の格子を、ページの画像(白地に濃い字、ざらつきつき)として、升目の中央からずらして置く。
    fn raster_of(map: &[f32], scale: usize, fade: f32) -> PageRaster {
        let off = (N - G) / 2 + 1;
        let (w, h) = (N * scale, N * scale);
        let mut rgba = vec![255u8; w * h * 4];
        let mut seed = 12345u32;
        for y in 0..h {
            for x in 0..w {
                let (gx, gy) = ((x / scale) as i32 - off as i32, (y / scale) as i32 - off as i32);
                let v = if (0..G as i32).contains(&gx) && (0..G as i32).contains(&gy) && !map.is_empty() { map[gy as usize * G + gx as usize] } else { 0.0 };
                // 紙のざらつき(±0.1 程度)を足す。
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = ((seed >> 16) as f32 / 65535.0 - 0.5) * 0.2;
                let g = (255.0 - (v * fade + noise).clamp(0.0, 1.0) * 255.0) as u8;
                let i = (y * w + x) * 4;
                rgba[i] = g;
                rgba[i + 1] = g;
                rgba[i + 2] = g;
            }
        }
        PageRaster { width: w, height: h, rgba, png: Vec::new(), px_to_user: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }
    }

    #[test]
    fn the_true_character_scores_higher_than_look_alikes_even_when_faded() {
        let Some(g) = font() else {
            return;
        };
        let pool: Vec<char> = "日曰目白毎朝持耕聞艇末".chars().collect();
        let index = GlyphIndex::build(&g, &pool);
        for truth in ['日', '毎', '朝', '持'] {
            // 字形の格子はぼかし済みで薄いので、強めに引き伸ばして「コントラスト補正後の字」にする。
            let page = raster_of(&g.render(0, truth).unwrap(), 3, 3.5);
            let cell = cell(&page, page.width as f32 / 2.0, page.height as f32 / 2.0, page.width as f32).unwrap();
            assert!(cell.evidence > 0.5, "{truth}: faded ink is still evidence ({})", cell.evidence);
            let (top, _) = index.rank_cpu(&cell.map, 1, &[]);
            assert_eq!(top[0].1, truth, "{truth} should be the best match, got {top:?}");
        }
    }

    #[test]
    fn an_empty_cell_has_no_evidence() {
        let page = raster_of(&vec![0.0; G * G], 3, 1.0); // ざらつきだけ
        let c = cell(&page, page.width as f32 / 2.0, page.height as f32 / 2.0, page.width as f32).unwrap();
        assert!(c.evidence < 0.05, "{}", c.evidence);
    }
}
