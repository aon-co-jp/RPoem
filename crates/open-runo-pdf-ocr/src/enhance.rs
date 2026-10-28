//! かすれた・薄い文字の読み取りを助ける前処理と再 OCR。
//!
//! 1. [`normalize_contrast`] — ページ全体が低コントラスト(薄いインク)のとき、濃淡を引き伸ばす(自動、常に安全)。
//!    これが無いと、薄い文書は「白紙」と判定されて OCR にすら送られないことがある。
//! 2. [`reocr_lines`] — 自信の低い行を切り出し、コントラストを整えて **AI 超解像(`open-runo-sr`)で 4 倍に拡大**してから、
//!    もう一度 OCR にかける。信頼度が明確に上がった行だけ採用する(設定 `reocr` でオン。CPU を使うため既定はオフ)。

use crate::client::OcrClient;
use crate::model::{OcrLine, OcrWord, Settings};
use crate::render::PageRaster;
use image::{ImageEncoder, RgbImage};

/// 画素(輝度)の分布から、背景(中央値)と最も濃いインク(0.3 パーセンタイル)を求める。
fn levels(rgba: &[u8], w: usize, h: usize) -> (u8, u8) {
    let mut hist = [0u32; 256];
    let mut n = 0u32;
    let mut i = 0;
    while i < w * h {
        let p = i * 4;
        let l = (299 * rgba[p] as u32 + 587 * rgba[p + 1] as u32 + 114 * rgba[p + 2] as u32) / 1000;
        hist[l.min(255) as usize] += 1;
        n += 1;
        i += 2;
    }
    if n == 0 {
        return (0, 255);
    }
    let (mut acc, mut ink, mut bg) = (0u32, 0u8, 255u8);
    let (mut got_ink, mut got_bg) = (false, false);
    for (l, &c) in hist.iter().enumerate() {
        acc += c;
        if !got_ink && (acc as f32) >= n as f32 * 0.003 {
            ink = l as u8;
            got_ink = true;
        }
        if !got_bg && acc * 2 >= n {
            bg = l as u8;
            got_bg = true;
        }
    }
    (ink, bg)
}

fn encode_png(rgba: &[u8], w: usize, h: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut out,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8)
    .map_err(|e| format!("PNG encode: {e}"))?;
    Ok(out)
}

/// ページが低コントラスト(背景と最も濃いインクの差が小さい)なら、濃淡を引き伸ばした新しいラスターを返す。
/// 十分なコントラストがあるとき(または、そもそも何も描かれていないとき)は `None`(何も変えない)。
pub fn normalize_contrast(r: &PageRaster) -> Option<PageRaster> {
    let (ink, bg) = levels(&r.rgba, r.width, r.height);
    let span = bg as i32 - ink as i32;
    // 差が 8 未満は、ほぼ真っ白(ノイズだけ)。110 以上は十分に濃い。
    if !(8..110).contains(&span) {
        return None;
    }
    let (lo, hi) = (ink as f32, bg as f32);
    let mut rgba = r.rgba.clone();
    for px in rgba.as_chunks_mut::<4>().0.iter_mut() {
        for c in px.iter_mut().take(3) {
            let v = ((*c as f32 - lo) / (hi - lo)).clamp(0.0, 1.0);
            // 少しだけ暗部を持ち上げず、背景は白へ。
            *c = (v * 255.0 + 0.5) as u8;
        }
    }
    let png = encode_png(&rgba, r.width, r.height).ok()?;
    Some(PageRaster {
        width: r.width,
        height: r.height,
        rgba,
        png,
        px_to_user: r.px_to_user,
    })
}

/// 切り抜きの濃淡を引き伸ばす(行ごとに、背景とインクの差を最大に)。
fn stretch(img: &mut RgbImage) {
    let rgba: Vec<u8> = img.pixels().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    let (ink, bg) = levels(&rgba, img.width() as usize, img.height() as usize);
    if (bg as i32 - ink as i32) < 8 {
        return;
    }
    let (lo, hi) = (ink as f32, bg as f32);
    for p in img.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = (((*c as f32 - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
}

fn is_weak(line: &OcrLine, below: f32) -> bool {
    let words: Vec<&OcrWord> = line
        .words
        .iter()
        .filter(|w| !w.text.trim().is_empty())
        .collect();
    if words.is_empty() {
        return true;
    }
    line.conf < below || words.iter().filter(|w| w.conf < 50.0).count() * 5 > words.len()
}

/// 再 OCR の読み方(拡大の方式・倍率・ノイズ除去の強さ)。
#[derive(Debug, Clone, Copy)]
struct Variant {
    ai: bool,
    scale: u32,
    denoise: f32,
}

/// 既定は 2 通りで読み、信頼度の高いほうを採る: AI 超解像で 3 倍(細部を補う)/ 高品質な補間で 3 倍(AI の作り込みを避ける)。
/// どちらもノイズを軽くならしてから拡大する(紙のざらつきを字形と取り違えないため)。実測(薄くぼけた文書)で、この組が最良だった。
const VARIANTS: [Variant; 2] = [
    Variant {
        ai: true,
        scale: 3,
        denoise: 1.2,
    },
    Variant {
        ai: false,
        scale: 3,
        denoise: 1.0,
    },
];

/// 1 行ぶんの切り抜き(周りに少し余白を足したもの)。
fn crop_line(raster: &PageRaster, l: &OcrLine) -> Option<(RgbImage, i32, i32)> {
    let pad = (l.h / 4).max(4);
    let x0 = (l.x - pad).max(0);
    let y0 = (l.y - pad).max(0);
    let x1 = (l.x + l.w + pad).min(raster.width as i32);
    let y1 = (l.y + l.h + pad).min(raster.height as i32);
    if x1 - x0 < 8 || y1 - y0 < 8 {
        return None;
    }
    let (cw, ch) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut crop = RgbImage::new(cw, ch);
    for yy in 0..ch {
        for xx in 0..cw {
            let p = (((y0 as u32 + yy) as usize * raster.width) + (x0 as u32 + xx) as usize) * 4;
            crop.put_pixel(
                xx,
                yy,
                image::Rgb([raster.rgba[p], raster.rgba[p + 1], raster.rgba[p + 2]]),
            );
        }
    }
    Some((crop, x0, y0))
}

const BORDER: u32 = 24;

/// 切り抜きを拡大して OCR に送れる PNG にする。
fn prepare(crop: &RgbImage, v: Variant) -> Result<Vec<u8>, String> {
    let mut c = crop.clone();
    stretch(&mut c);
    if v.denoise > 0.0 {
        c = image::imageops::blur(&c, v.denoise);
    }
    let big = open_runo_sr::upscale_crop(&c, v.scale, v.ai, 0)?;
    // 白い縁を付ける(Tesseract は余白があるほうが安定する)。
    let (bw, bh) = (big.width() + BORDER * 2, big.height() + BORDER * 2);
    let mut framed = RgbImage::from_pixel(bw, bh, image::Rgb([255, 255, 255]));
    image::imageops::replace(&mut framed, &big, BORDER as i64, BORDER as i64);
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(framed.as_raw(), bw, bh, image::ExtendedColorType::Rgb8)
    .map_err(|e| format!("PNG encode: {e}"))?;
    Ok(png)
}

/// 自信の低い行を、ノイズ除去 → 拡大(AI 超解像 / 補間)→ 再 OCR で読み直す。採用した行の数を返す。
/// 行の位置・大きさは元のまま、文字列・単語・信頼度だけを置き換える。複数の読み方のうち最も信頼度が高いものを、
/// 元の読みより明確に良い(信頼度 +3 以上、文字数が極端に違わない)ときだけ採用する。
pub fn reocr_lines(
    client: &OcrClient,
    raster: &PageRaster,
    lines: &mut [OcrLine],
    settings: &Settings,
) -> Result<usize, String> {
    let debug = std::env::var_os("PDFOCR_DEBUG").is_some();
    let weak: Vec<usize> = (0..lines.len())
        .filter(|&i| is_weak(&lines[i], settings.reocr_below))
        .take(settings.reocr_max_lines)
        .collect();
    if weak.is_empty() {
        return Ok(0);
    }
    let crops: Vec<(usize, RgbImage, i32, i32)> = weak
        .iter()
        .filter_map(|&i| crop_line(raster, &lines[i]).map(|(c, x0, y0)| (i, c, x0, y0)))
        .collect();
    // 各行について、全ての読み方の結果から、最も信頼度の高いものを選ぶ。
    let mut best: Vec<Option<(OcrLine, Variant, i32, i32)>> = vec![None; crops.len()];
    for v in VARIANTS {
        let mut pngs = Vec::with_capacity(crops.len());
        for (_, crop, _, _) in &crops {
            pngs.push(prepare(crop, v)?);
        }
        for (ci, chunk) in pngs.chunks(8).enumerate() {
            let results = client.ocr(chunk, &settings.languages, settings.quality)?;
            for (k, found) in results.into_iter().enumerate() {
                let idx = ci * 8 + k;
                let (li, _, x0, y0) = &crops[idx];
                let l = &lines[*li];
                // 切り抜きの中央に最も近い行を、その行の再読み取りとして使う(周りの行が少し写り込んでいても無視する)。
                let center_y = ((l.y + l.h / 2 - y0) as f32) * v.scale as f32 + BORDER as f32;
                let Some(pick) = found
                    .into_iter()
                    .filter(|f| !f.text.trim().is_empty())
                    .min_by(|a, b| {
                        (a.y as f32 + a.h as f32 / 2.0 - center_y)
                            .abs()
                            .total_cmp(&(b.y as f32 + b.h as f32 / 2.0 - center_y).abs())
                    })
                else {
                    continue;
                };
                if debug {
                    eprintln!(
                        "[reocr] line {li} ai={} x{} conf={:.0} {:?}",
                        v.ai, v.scale, pick.conf, pick.text
                    );
                }
                if best[idx].as_ref().is_none_or(|(b, ..)| pick.conf > b.conf) {
                    best[idx] = Some((pick, v, *x0, *y0));
                }
            }
        }
    }
    let mut adopted = 0;
    for (idx, found) in best.into_iter().enumerate() {
        let Some((pick, v, x0, y0)) = found else {
            continue;
        };
        let li = crops[idx].0;
        let old = &lines[li];
        let (old_n, new_n) = (
            old.text.chars().count() as f32,
            pick.text.chars().count() as f32,
        );
        let plausible_len = old_n == 0.0 || (new_n >= old_n * 0.6 && new_n <= old_n * 1.6);
        let better = pick.conf >= old.conf + 3.0 || old.text.trim().is_empty();
        if !(plausible_len && better) {
            continue;
        }
        let s = v.scale as f32;
        let map_x = |x: i32| x0 + ((x as f32 - BORDER as f32) / s).round() as i32;
        let map_y = |y: i32| y0 + ((y as f32 - BORDER as f32) / s).round() as i32;
        let words: Vec<OcrWord> = pick
            .words
            .iter()
            .map(|w| OcrWord {
                text: w.text.clone(),
                x: map_x(w.x),
                y: map_y(w.y),
                w: (w.w as f32 / s).round() as i32,
                h: (w.h as f32 / s).round() as i32,
                conf: w.conf,
            })
            .collect();
        let l = &mut lines[li];
        l.text = pick.text.clone();
        l.conf = pick.conf;
        l.words = words;
        adopted += 1;
    }
    Ok(adopted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(bg: u8, ink: u8) -> PageRaster {
        let (w, h) = (100usize, 100usize);
        let mut rgba = vec![bg; w * h * 4];
        for px in rgba.as_chunks_mut::<4>().0.iter_mut() {
            px[3] = 255;
        }
        // 細い線(全体の 1% 強)を引く。
        for y in 39..45 {
            for x in 10..90 {
                let p = (y * w + x) * 4;
                rgba[p] = ink;
                rgba[p + 1] = ink;
                rgba[p + 2] = ink;
            }
        }
        PageRaster {
            width: w,
            height: h,
            rgba,
            png: Vec::new(),
            px_to_user: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    #[test]
    fn faint_pages_are_stretched_and_good_pages_are_left_alone() {
        let faint = raster(240, 190);
        let out = normalize_contrast(&faint).expect("a faint page should be normalized");
        let p = (40 * 100 + 50) * 4;
        assert!(out.rgba[p] < 30, "ink becomes dark: {}", out.rgba[p]);
        assert!(out.rgba[0] > 240, "background stays white");
        assert!(!out.png.is_empty());
        assert!(
            normalize_contrast(&raster(250, 20)).is_none(),
            "a normal page is not touched"
        );
        assert!(
            normalize_contrast(&raster(255, 252)).is_none(),
            "an empty page stays empty"
        );
    }

    #[test]
    fn a_normalized_faint_page_is_no_longer_blank() {
        let faint = raster(240, 195);
        assert!(
            faint.is_blank(),
            "the faint page looks blank to the plain detector"
        );
        let out = normalize_contrast(&faint).unwrap();
        assert!(!out.is_blank());
    }
}
