//! 行の見た目の判定: 書体の系統(ゴシック体/明朝体)・太字・文字色・背景色。
//!
//! ページ画像から、OCR が見つけた行の矩形の中を調べる。
//! - 背景色: 矩形の外周の中央値。
//! - 文字(インク)の画素: 大津の方法で二値化。文字色は最も濃い画素の平均。
//! - 明朝体: 横画が縦画より細い(筆圧のコントラストが高い)。横方向・縦方向の連続画素の長さの中央値の比で判定。
//! - 太字: 縦画の太さが行の高さに対して十分に太い。
//!
//! いずれも画像からの**推定**であり、完全には一致しない場合がある。

/// 行の見た目の推定結果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineStyle {
    /// 明朝体(セリフ体)らしいか。偽ならゴシック体(サンセリフ)。
    pub serif: bool,
    /// 太字らしいか。
    pub bold: bool,
    /// 文字色(RGB)。
    pub fg: [u8; 3],
    /// 背景色(RGB)。
    pub bg: [u8; 3],
    /// 文字(インク)が十分に見つかったか。偽なら置き換えない方がよい。
    pub has_ink: bool,
    /// 横画/縦画の太さの比(明朝体判定の根拠。小さいほど明朝体らしい)。
    pub contrast: f32,
    /// 縦方向の連続画素(横画の太さ)のばらつき(変動係数)。明朝体は太い画と細い画が混ざり大きくなる。
    pub cv_bar: f32,
    /// 横方向の連続画素(縦画の太さ)のばらつき(変動係数)。
    pub cv_stem: f32,
    /// 縦画の太さ/行の高さ(太字判定の根拠)。
    pub stem_ratio: f32,
    /// 縦画の太さ(px)。
    pub stem_px: f32,
}

/// 日本語などの太字と判定する、縦画の太さ/em の下限(ゴシック体)。
pub const BOLD_EM_SANS: f32 = 0.112;
/// 同(明朝体)。
pub const BOLD_EM_SERIF: f32 = 0.086;

impl LineStyle {
    /// 行の文字列(日本語などの CJK か、英数字か)に合わせて、明朝体・太字を判定する。
    ///
    /// 基準は、書体の分かっている見本(Arial・Verdana・Segoe UI・Calibri・Times・Georgia・Cambria・
    /// Yu Gothic/Mincho・Meiryo・MS Gothic/Mincho・Malgun Gothic・YaHei の各サイズ・標準/太字)での実測から決めた。
    /// - 英数字: 縦画の太さ/行の高さ が 0.13 を超えれば太字。横画/縦画の比は、標準で 0.90 未満・太字で 0.78 未満なら明朝体(セリフ体)。
    /// - 日本語など(CJK): 太字は 縦画の太さ/em(行の幅と文字数から推定)が 0.112(明朝体は 0.086)を超えるとき。明朝体は、横画が細く(比 0.86 未満)かつ画の太さのばらつきが大きい(0.70 超)ときだけ。
    ///   CJK の明朝体は画の太さだけでは区別しにくいため、確信が高いときだけ明朝体とし、迷えばゴシック体にする。
    pub fn classify(mut self, text: &str, line_w: i32) -> LineStyle {
        if !self.has_ink {
            return self;
        }
        let total = text.chars().filter(|c| !c.is_whitespace()).count().max(1);
        let cjk_chars = text
            .chars()
            .filter(|&c| {
                crate::fonts::is_kana(c)
                    || crate::fonts::is_hangul(c)
                    || crate::fonts::is_han(c)
                    || crate::fonts::is_cjk_symbol(c)
            })
            .count();
        let cjk = cjk_chars * 2 >= total;
        if cjk {
            // 日本語などは全ての文字の送り幅が 1em なので、行の幅と文字数から文字の大きさ(em)を推定し、
            // 縦画の太さをそれに対する比で見る(OCR の行の高さは文字より大きくなりがちなため)。
            let em =
                (line_w as f32 / (cjk_chars as f32 + 0.55 * (total - cjk_chars) as f32)).max(4.0);
            let stem_em = self.stem_px / em;
            self.serif = self.contrast < 0.86 && self.cv_bar > 0.70;
            self.bold = stem_em
                > if self.serif {
                    BOLD_EM_SERIF
                } else {
                    BOLD_EM_SANS
                };
        } else {
            self.bold = self.stem_ratio > 0.13;
            self.serif = self.contrast < if self.bold { 0.78 } else { 0.90 };
        }
        self
    }
}

fn lum(p: &[u8]) -> i32 {
    (299 * p[0] as i32 + 587 * p[1] as i32 + 114 * p[2] as i32) / 1000
}

/// `rgba`(幅 `w`・高さ `h`)上の矩形 `(x, y, rw, rh)` を調べる。
#[allow(clippy::too_many_arguments)]
pub fn analyze(rgba: &[u8], w: usize, h: usize, x: i32, y: i32, rw: i32, rh: i32) -> LineStyle {
    let blank = LineStyle {
        serif: false,
        bold: false,
        fg: [0, 0, 0],
        bg: [255, 255, 255],
        has_ink: false,
        contrast: 1.0,
        cv_bar: 0.0,
        cv_stem: 0.0,
        stem_ratio: 0.0,
        stem_px: 0.0,
    };
    let x0 = x.clamp(0, w as i32 - 1);
    let y0 = y.clamp(0, h as i32 - 1);
    let x1 = (x + rw).clamp(x0 + 1, w as i32);
    let y1 = (y + rh).clamp(y0 + 1, h as i32);
    let cw = (x1 - x0) as usize;
    let ch = (y1 - y0) as usize;
    if cw < 2 || ch < 4 {
        return blank;
    }
    let px = |xx: usize, yy: usize| -> &[u8] {
        let i = (yy * w + xx) * 4;
        &rgba[i..i + 4]
    };

    // 背景色: 矩形の外周(幅 pad)の画素の、輝度の中央値の画素。
    let pad = ((ch as i32) / 6).max(2);
    let mut ring: Vec<(i32, [u8; 3])> = Vec::new();
    let rx0 = (x0 - pad).max(0);
    let ry0 = (y0 - pad).max(0);
    let rx1 = (x1 + pad).min(w as i32);
    let ry1 = (y1 + pad).min(h as i32);
    for yy in ry0..ry1 {
        for xx in rx0..rx1 {
            let inside = xx >= x0 && xx < x1 && yy >= y0 && yy < y1;
            if inside || (xx + yy) % 3 != 0 {
                continue;
            }
            let p = px(xx as usize, yy as usize);
            ring.push((lum(p), [p[0], p[1], p[2]]));
        }
    }
    let bg = if ring.is_empty() {
        [255, 255, 255]
    } else {
        ring.sort_unstable_by_key(|r| r.0);
        ring[ring.len() / 2].1
    };
    let bg_lum = lum(&[bg[0], bg[1], bg[2], 255]);

    // 矩形内の輝度ヒストグラムから大津の閾値。
    let mut hist = [0u32; 256];
    let mut lums = Vec::with_capacity(cw * ch);
    for yy in 0..ch {
        for xx in 0..cw {
            let l = lum(px(x0 as usize + xx, y0 as usize + yy)).clamp(0, 255);
            hist[l as usize] += 1;
            lums.push(l as u8);
        }
    }
    let total = (cw * ch) as f64;
    let sum_all: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * c as f64)
        .sum();
    let (mut wb, mut sb, mut best, mut thr) = (0.0f64, 0.0f64, -1.0f64, 128u8);
    for (t, &c) in hist.iter().enumerate() {
        wb += c as f64;
        if wb == 0.0 {
            continue;
        }
        let wf = total - wb;
        if wf == 0.0 {
            break;
        }
        sb += t as f64 * c as f64;
        let (mb, mf) = (sb / wb, (sum_all - sb) / wf);
        let var = wb * wf * (mb - mf) * (mb - mf);
        if var > best {
            best = var;
            thr = t as u8;
        }
    }
    // 背景より十分に暗い(または明るい)画素だけをインクとする。背景が暗い(白抜き文字)場合は反転。
    let dark_text = bg_lum >= 128;
    let min_diff = 40;
    let is_ink = |l: u8| -> bool {
        if dark_text {
            l <= thr && bg_lum - (l as i32) >= min_diff
        } else {
            l > thr && (l as i32) - bg_lum >= min_diff
        }
    };
    let mask: Vec<bool> = lums.iter().map(|&l| is_ink(l)).collect();
    let ink_count = mask.iter().filter(|&&m| m).count();
    if ink_count * 1000 < cw * ch * 3 {
        return LineStyle { bg, ..blank };
    }

    // 文字色: インク画素のうち、背景から最も離れた上位 20% の平均。
    let mut ink_px: Vec<(i32, [u8; 3])> = Vec::with_capacity(ink_count);
    for yy in 0..ch {
        for xx in 0..cw {
            if mask[yy * cw + xx] {
                let p = px(x0 as usize + xx, y0 as usize + yy);
                ink_px.push(((lum(p) - bg_lum).abs(), [p[0], p[1], p[2]]));
            }
        }
    }
    ink_px.sort_unstable_by_key(|p| -p.0);
    let take = (ink_px.len() / 5).max(1);
    let mut sum = [0u64; 3];
    for p in ink_px.iter().take(take) {
        for (s, v) in sum.iter_mut().zip(p.1) {
            *s += v as u64;
        }
    }
    let fg = [
        (sum[0] / take as u64) as u8,
        (sum[1] / take as u64) as u8,
        (sum[2] / take as u64) as u8,
    ];

    // 画の太さ: 横方向の連続画素(=縦画の太さ)と縦方向の連続画素(=横画の太さ)の長さの中央値。
    // 行の高さの 35% を超える連続(縦画全体・長い横棒)は除く。
    let limit = ((ch as f32) * 0.35).max(2.0) as usize;
    let mut h_runs: Vec<i32> = Vec::new();
    for yy in 0..ch {
        let mut run = 0usize;
        for xx in 0..=cw {
            if xx < cw && mask[yy * cw + xx] {
                run += 1;
            } else {
                if run > 0 && run <= limit {
                    h_runs.push(run as i32);
                }
                run = 0;
            }
        }
    }
    let mut v_runs: Vec<i32> = Vec::new();
    for xx in 0..cw {
        let mut run = 0usize;
        for yy in 0..=ch {
            if yy < ch && mask[yy * cw + xx] {
                run += 1;
            } else {
                if run > 0 && run <= limit {
                    v_runs.push(run as i32);
                }
                run = 0;
            }
        }
    }
    let mean = |v: &[i32]| -> f32 {
        if v.is_empty() {
            1.0
        } else {
            v.iter().sum::<i32>() as f32 / v.len() as f32
        }
    };
    let stem = mean(&h_runs).max(1.0); // 縦画の太さ(平均)
    let bar = mean(&v_runs).max(1.0); // 横画の太さ(平均)
    let contrast = bar / stem;
    let cv = |v: &[i32], m: f32| -> f32 {
        if v.len() < 2 {
            0.0
        } else {
            (v.iter().map(|&x| (x as f32 - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt() / m
        }
    };
    let (cv_bar, cv_stem) = (cv(&v_runs, bar), cv(&h_runs, stem));
    let stem_ratio = stem / ch as f32;
    LineStyle {
        serif: false,
        bold: false,
        fg,
        bg,
        has_ink: true,
        contrast,
        cv_bar,
        cv_stem,
        stem_ratio,
        stem_px: stem,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_area_has_no_ink() {
        let rgba = vec![255u8; 100 * 40 * 4];
        let s = analyze(&rgba, 100, 40, 10, 10, 60, 20);
        assert!(!s.has_ink);
    }

    #[test]
    fn thick_vertical_bars_look_like_gothic_and_bold() {
        // 黒い縦棒(太さ 5px、高さ 20px)を等間隔に並べた白背景。
        let (w, h) = (120usize, 40usize);
        let mut rgba = vec![255u8; w * h * 4];
        for bar in 0..8 {
            for yy in 10..30 {
                for xx in 0..5 {
                    let i = (yy * w + 10 + bar * 12 + xx) * 4;
                    rgba[i] = 0;
                    rgba[i + 1] = 0;
                    rgba[i + 2] = 0;
                }
            }
        }
        let s = analyze(&rgba, w, h, 8, 8, 100, 24);
        assert!(s.has_ink);
        let s = s.classify("A", 100);
        assert!(s.bold, "stem_ratio={}", s.stem_ratio);
        assert!(s.fg[0] < 30 && s.bg[0] > 240);
    }
}
