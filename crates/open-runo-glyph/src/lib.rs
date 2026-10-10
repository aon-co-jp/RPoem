//! OCR 用の字形(グリフ)照合の共有ライブラリ。
//!
//! かすれた文字の升目(ページ画像の切り抜き)と、候補の文字をフォントで描いた字形を、正規化相互相関(NCC)で比べる。
//! 升目の中で字形をずらして最も合う位置を探すため、「升目の全ての窓(`WINDOWS` 個)」と「全ての候補の字形」の
//! 内積(= 行列積)として計算できる。[`GlyphIndex`] は候補の字形を 1 つの行列にまとめ、[`windows`] は升目を窓の行列にする。
//! 行列積は CPU の素朴な実装([`GlyphIndex::rank_cpu`])でも、`open-cuda` / `open-cpu` の `sgemm` でも実行できる
//! (`sgemm` に渡す行列は [`GlyphIndex::matrix`] と [`windows`] の戻り値)。

use std::collections::HashMap;

/// 升目(ページから切り出した範囲)の格子の一辺(画素)。字 1 文字の 2 倍の範囲を切り出す。
pub const N: usize = 32;
/// 字形の格子の一辺(画素)。升目の中で、これを動かして最も合う位置を探す。
pub const G: usize = 16;
/// 升目の中の窓(字形を重ねる位置)の数の一辺。
pub const OFFSETS: usize = N - G + 1;
/// 窓の数。
pub const WINDOWS: usize = OFFSETS * OFFSETS;
/// 窓・字形の 1 つあたりの要素数。
pub const K: usize = G * G;

/// 3×3 の平均(小さなずれ・かすれを許すため)。`n` は格子の一辺。
pub fn blur3(a: &[f32], n: usize) -> Vec<f32> {
    let mut out = vec![0f32; n * n];
    for y in 0..n {
        for x in 0..n {
            let (mut s, mut c) = (0.0, 0.0);
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                    if (0..n as i32).contains(&xx) && (0..n as i32).contains(&yy) {
                        s += a[yy as usize * n + xx as usize];
                        c += 1.0;
                    }
                }
            }
            out[y * n + x] = s / c;
        }
    }
    out
}

/// 平均を引いて、長さ 1 に正規化する(NCC を内積にするため)。分散が無ければ 0 のまま。
fn center_unit(v: &mut [f32]) {
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    let mut norm = 0.0f32;
    for x in v.iter_mut() {
        *x -= mean;
        norm += *x * *x;
    }
    if norm > 1e-9 {
        let inv = 1.0 / norm.sqrt();
        v.iter_mut().for_each(|x| *x *= inv);
    } else {
        v.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// 升目(`N`×`N`)を、字形(`G`×`G`)を重ねる全ての位置の窓に分け、各窓を平均 0・長さ 1 にした行列
/// (`WINDOWS` 行 × `K` 列、行優先)にする。
pub fn windows(cell: &[f32]) -> Vec<f32> {
    let mut out = vec![0f32; WINDOWS * K];
    for oy in 0..OFFSETS {
        for ox in 0..OFFSETS {
            let row = &mut out[(oy * OFFSETS + ox) * K..(oy * OFFSETS + ox + 1) * K];
            for y in 0..G {
                row[y * G..(y + 1) * G].copy_from_slice(&cell[(oy + y) * N + ox..(oy + y) * N + ox + G]);
            }
            center_unit(row);
        }
    }
    out
}

/// フォント(複数可)から字形の格子を作る(同じ文字は覚えておく)。
pub struct Glyphs {
    fonts: Vec<fontdue::Font>,
}

impl Glyphs {
    /// 複数のフォント(例: 明朝体とゴシック体)で作る。1 つも読めなければ `None`。
    pub fn with_fonts(fonts: &[&[u8]]) -> Option<Self> {
        let fonts: Vec<fontdue::Font> = fonts.iter().filter_map(|b| fontdue::Font::from_bytes(*b, fontdue::FontSettings::default()).ok()).collect();
        if fonts.is_empty() {
            None
        } else {
            Some(Glyphs { fonts })
        }
    }

    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// `font` 番目のフォントでの文字の字形(`G`×`G`、0〜1、ぼかし済み)。フォントに無い文字は `None`。
    pub fn render(&self, font: usize, ch: char) -> Option<Vec<f32>> {
        let f = self.fonts.get(font)?;
        if f.lookup_glyph_index(ch) == 0 {
            return None;
        }
        let px = G as f32 * 0.86;
        let (m, bmp) = f.rasterize(ch, px);
        let lm = f.horizontal_line_metrics(px)?;
        let mut out = vec![0f32; G * G];
        // 字の升(上端 = ascent、下端 = descent)を格子の中央に置く。
        let em_h = lm.ascent - lm.descent;
        let baseline = (G as f32 - em_h) / 2.0 + lm.ascent;
        let x0 = ((G as f32 - m.advance_width) / 2.0 + m.xmin as f32).round() as i32;
        let y0 = (baseline - (m.ymin as f32 + m.height as f32)).round() as i32;
        for gy in 0..m.height {
            for gx in 0..m.width {
                let (x, y) = (x0 + gx as i32, y0 + gy as i32);
                if (0..G as i32).contains(&x) && (0..G as i32).contains(&y) {
                    out[y as usize * G + x as usize] = bmp[gy * m.width + gx] as f32 / 255.0;
                }
            }
        }
        Some(blur3(&out, G))
    }
}

/// 候補の文字の字形を 1 つの行列にまとめたもの(各列が 1 つの候補の字形 = 平均 0・長さ 1)。
pub struct GlyphIndex {
    /// 列 `j` が表す文字(同じ文字が複数のフォントで入ることがある)。
    chars: Vec<char>,
    /// `K` 行 × `chars.len()` 列、行優先。
    matrix: Vec<f32>,
}

impl GlyphIndex {
    /// `pool` の全ての文字を、全てのフォントで描いて行列にする(フォントに無い文字は入らない)。レンダリングはスレッドで分担する。
    pub fn build(glyphs: &Glyphs, pool: &[char]) -> Self {
        let jobs: Vec<(usize, char)> = (0..glyphs.font_count()).flat_map(|f| pool.iter().map(move |&c| (f, c))).collect();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 16);
        let chunk = jobs.len().div_ceil(threads).max(1);
        let rendered: Vec<(char, Vec<f32>)> = std::thread::scope(|sc| {
            let handles: Vec<_> = jobs
                .chunks(chunk)
                .map(|part| {
                    sc.spawn(move || {
                        part.iter()
                            .filter_map(|&(f, c)| {
                                let mut g = glyphs.render(f, c)?;
                                center_unit(&mut g);
                                (g.iter().any(|&v| v != 0.0)).then_some((c, g))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        let m = rendered.len();
        let mut matrix = vec![0f32; K * m];
        let mut chars = Vec::with_capacity(m);
        for (j, (c, g)) in rendered.into_iter().enumerate() {
            for (k, v) in g.into_iter().enumerate() {
                matrix[k * m + j] = v;
            }
            chars.push(c);
        }
        GlyphIndex { chars, matrix }
    }

    /// 候補(列)の数。
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// `K` 行 × `len()` 列の行列(行優先)。`sgemm` の `B` に渡す。
    pub fn matrix(&self) -> &[f32] {
        &self.matrix
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    /// 窓 × 候補の得点行列(`WINDOWS` 行 × `len()` 列、行優先)から、候補ごとの最大(最も合う位置)を取り、
    /// 文字ごとに(複数のフォントのうち)最大にまとめて、上位 `top` 個と、`query` の文字の得点を返す。
    pub fn reduce(&self, scores: &[f32], top: usize, query: &[char]) -> (Vec<(f32, char)>, Vec<(char, f32)>) {
        let m = self.len();
        let mut per_col = vec![-1.0f32; m];
        for w in 0..WINDOWS {
            let row = &scores[w * m..(w + 1) * m];
            for (best, &v) in per_col.iter_mut().zip(row) {
                if v > *best {
                    *best = v;
                }
            }
        }
        let mut per_char: HashMap<char, f32> = HashMap::new();
        for (j, &c) in self.chars.iter().enumerate() {
            let e = per_char.entry(c).or_insert(-1.0);
            if per_col[j] > *e {
                *e = per_col[j];
            }
        }
        let mut ranked: Vec<(f32, char)> = per_char.iter().map(|(&c, &v)| (v, c)).collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        ranked.truncate(top);
        let q = query.iter().filter_map(|c| per_char.get(c).map(|&v| (*c, v))).collect();
        (ranked, q)
    }

    /// CPU だけで(外部の行列積に頼らず)計算する参照実装。スレッドで候補を分担する。
    pub fn rank_cpu(&self, cell: &[f32], top: usize, query: &[char]) -> (Vec<(f32, char)>, Vec<(char, f32)>) {
        let m = self.len();
        let win = windows(cell);
        let mut scores = vec![0f32; WINDOWS * m];
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 16);
        let rows_per = WINDOWS.div_ceil(threads).max(1);
        std::thread::scope(|sc| {
            for (ci, part) in scores.chunks_mut(rows_per * m).enumerate() {
                let win = &win;
                let matrix = &self.matrix;
                sc.spawn(move || {
                    for (ri, row) in part.chunks_mut(m).enumerate() {
                        let w = &win[(ci * rows_per + ri) * K..(ci * rows_per + ri + 1) * K];
                        for (k, &wv) in w.iter().enumerate() {
                            if wv == 0.0 {
                                continue;
                            }
                            let brow = &matrix[k * m..(k + 1) * m];
                            for (o, &b) in row.iter_mut().zip(brow) {
                                *o += wv * b;
                            }
                        }
                    }
                });
            }
        });
        self.reduce(&scores, top, query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Option<Glyphs> {
        for p in ["F:/tessdata-jpn/fonts/NotoSansJP-VF.ttf", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"] {
            if let Ok(b) = std::fs::read(p) {
                if let Some(g) = Glyphs::with_fonts(&[&b[..]]) {
                    return Some(g);
                }
            }
        }
        None
    }

    /// 字形(`G`×`G`)を、`N`×`N` の升目の中央からずらして置く。
    fn cell_with(glyph: &[f32], dx: usize, dy: usize) -> Vec<f32> {
        let mut c = vec![0f32; N * N];
        for y in 0..G {
            for x in 0..G {
                c[(y + dy) * N + x + dx] = glyph[y * G + x];
            }
        }
        c
    }

    #[test]
    fn windows_are_centered_and_unit_length() {
        let mut cell = vec![0f32; N * N];
        for (i, v) in cell.iter_mut().enumerate() {
            *v = ((i * 7) % 13) as f32 / 13.0;
        }
        let w = windows(&cell);
        assert_eq!(w.len(), WINDOWS * K);
        for row in w.chunks(K) {
            let mean: f32 = row.iter().sum::<f32>() / K as f32;
            let norm: f32 = row.iter().map(|v| v * v).sum::<f32>();
            assert!(mean.abs() < 1e-4 && (norm - 1.0).abs() < 1e-3, "mean {mean} norm {norm}");
        }
    }

    #[test]
    fn the_true_character_is_found_at_any_offset_and_beats_look_alikes() {
        let Some(g) = font() else {
            return;
        };
        let pool: Vec<char> = "日曰目白毎朝耕聞末未持特待".chars().collect();
        let index = GlyphIndex::build(&g, &pool);
        assert_eq!(index.len(), pool.len());
        for truth in ['日', '毎', '朝', '持'] {
            for (dx, dy) in [(8, 8), (3, 11), (13, 2)] {
                let cell = cell_with(&g.render(0, truth).unwrap(), dx, dy);
                let (top, q) = index.rank_cpu(&cell, 3, &[truth, '曰']);
                assert_eq!(top[0].1, truth, "offset ({dx},{dy}): {top:?}");
                assert!(top[0].0 > 0.99, "an exact copy correlates perfectly: {}", top[0].0);
                let own = q.iter().find(|(c, _)| *c == truth).unwrap().1;
                assert!(own >= q.iter().map(|(_, v)| *v).fold(-1.0, f32::max) - 1e-6);
            }
        }
    }

    #[test]
    fn a_matrix_product_gives_the_same_scores_as_the_reference() {
        let Some(g) = font() else {
            return;
        };
        let pool: Vec<char> = "日曰目白毎朝耕聞".chars().collect();
        let index = GlyphIndex::build(&g, &pool);
        let cell = cell_with(&g.render(0, '毎').unwrap(), 5, 9);
        // 外部の sgemm の代わりに、素朴な 3 重ループ(row-major の C = A·B)で同じ計算をして、reduce の結果を比べる。
        let a = windows(&cell);
        let (m, b) = (index.len(), index.matrix());
        let mut c = vec![0f32; WINDOWS * m];
        for i in 0..WINDOWS {
            for k in 0..K {
                for j in 0..m {
                    c[i * m + j] += a[i * K + k] * b[k * m + j];
                }
            }
        }
        let (top_a, _) = index.reduce(&c, 3, &[]);
        let (top_b, _) = index.rank_cpu(&cell, 3, &[]);
        assert_eq!(top_a.iter().map(|t| t.1).collect::<Vec<_>>(), top_b.iter().map(|t| t.1).collect::<Vec<_>>());
        assert!((top_a[0].0 - top_b[0].0).abs() < 1e-5);
    }
}
