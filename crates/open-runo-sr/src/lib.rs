//! 写真・画像の高解像度化(アップコンバート)の共有クレート。
//!
//! 画像を Full HD / QHD / 4K / 5K / 8K / 16K の枠に合わせて拡大する。拡大には 2 つの方法がある:
//! - **AI 超解像**: Real-ESRGAN `realesr-general-x4v3`(CPU 版、`make-disk` から移植)で 4 倍に拡大して細部を補い、
//!   残りの倍率は高品質な補間(Lanczos)で目標の大きさに合わせる。**時間がかかる**(CPU・画素数次第)。
//! - **高品質な補間のみ**: Lanczos による拡大。速いが、失われた細部は補わない。
//!
//! 画像は縦横比を保ち、目標の枠の**内側**に収まる最大の大きさにする(切り取らない)。
//! 大きな画像で使うメモリが増えすぎないよう、AI 超解像は横長の帯に分けて処理する。
//!
//! **正直な開示**: AI 超解像は「失われた細部を推測して補う」ものであり、実際にあった細部の復元ではない
//! (質感が不自然になることがある)。Arm の CPU(スマホ・Apple Silicon)では AVX2 が無くスカラー実装になり、とても遅い。
//! 8K・16K は、元の画像より大幅に大きくなるため、出力のファイルサイズ・メモリ・時間が大きい。

#![allow(unsafe_code, clippy::needless_range_loop, clippy::too_many_arguments)]

mod model;

pub use model::{kernel_name, SrModel};

use image::{DynamicImage, ImageEncoder, RgbImage};

/// 同梱の AI モデル(`realesr-general-x4v3`、Real-ESRGAN、BSD-3-Clause)。
const PARAM: &str = include_str!("../models/realesr-general-x4v3.param");
const BIN: &[u8] = include_bytes!("../models/realesr-general-x4v3.bin");

/// 目標の解像度(枠)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Full HD 1920×1080。
    FullHd,
    /// QHD 2560×1440(Full HD と 4K の間)。
    Qhd,
    /// 4K UHD 3840×2160。
    Uhd4k,
    /// 5K 5120×2880(4K と 8K の間)。
    Uhd5k,
    /// 8K UHD 7680×4320。
    Uhd8k,
    /// 16K 15360×8640。
    Uhd16k,
}

impl Target {
    /// 全ての目標(小さい順)。
    pub const ALL: [Target; 6] = [Target::FullHd, Target::Qhd, Target::Uhd4k, Target::Uhd5k, Target::Uhd8k, Target::Uhd16k];

    /// 枠の幅・高さ(横長)。
    pub fn frame(self) -> (u32, u32) {
        match self {
            Target::FullHd => (1920, 1080),
            Target::Qhd => (2560, 1440),
            Target::Uhd4k => (3840, 2160),
            Target::Uhd5k => (5120, 2880),
            Target::Uhd8k => (7680, 4320),
            Target::Uhd16k => (15360, 8640),
        }
    }

    /// 表示名(日本語, 英語)。
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            Target::FullHd => ("フルHD 1920×1080", "Full HD 1920×1080"),
            Target::Qhd => ("QHD 2560×1440(フルHDと4Kの間)", "QHD 2560×1440 (between Full HD and 4K)"),
            Target::Uhd4k => ("4K 3840×2160", "4K 3840×2160"),
            Target::Uhd5k => ("5K 5120×2880(4Kと8Kの間)", "5K 5120×2880 (between 4K and 8K)"),
            Target::Uhd8k => ("8K 7680×4320", "8K 7680×4320"),
            Target::Uhd16k => ("16K 15360×8640", "16K 15360×8640"),
        }
    }
}

/// 元の画像の大きさを、目標の枠に縦横比を保って収めたときの出力の大きさ。
/// 縦長の画像は、枠を縦横入れ替えて(縦長の枠として)収める。
pub fn output_size(src_w: u32, src_h: u32, target: Target) -> (u32, u32) {
    let (fw, fh) = target.frame();
    let (fw, fh) = if src_h > src_w { (fh, fw) } else { (fw, fh) };
    let scale = (fw as f64 / src_w.max(1) as f64).min(fh as f64 / src_h.max(1) as f64);
    (((src_w as f64 * scale).round() as u32).max(1), ((src_h as f64 * scale).round() as u32).max(1))
}

/// 出力の形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// 可逆の PNG(大きい)。
    Png,
    /// JPEG(品質 92)。
    Jpeg,
}

/// 変換の指定。
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 目標の解像度。
    pub target: Target,
    /// AI 超解像を使うか(偽なら高品質な補間のみ)。
    pub ai: bool,
    /// 出力の形式。
    pub format: Format,
    /// 使うスレッド数(0 = 全て)。
    pub threads: usize,
}

/// 進捗(0.0〜1.0)と段階の説明。
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// 全体の進み具合。
    pub fraction: f32,
    /// いま何をしているか(日英)。
    pub stage: &'static str,
}

/// 結果。
#[derive(Debug, Clone)]
pub struct Output {
    /// 出力の画像ファイル(PNG または JPEG のバイト列)。
    pub bytes: Vec<u8>,
    /// 出力の幅。
    pub width: u32,
    /// 出力の高さ。
    pub height: u32,
    /// 実際に AI 超解像を使ったか(目標が元より小さい、など AI が不要なときは偽)。
    pub used_ai: bool,
}

const BAND_ROWS: usize = 256;

fn load_model() -> Result<SrModel, String> {
    model::parse_model(PARAM, BIN)
}

/// 画像のバイト列(PNG/JPEG)を、`opts` に従って高解像度化する。
pub fn upscale(input: &[u8], opts: Options, progress: &mut dyn FnMut(Progress), cancelled: &dyn Fn() -> bool) -> Result<Output, String> {
    let cancel = || "キャンセルしました / Cancelled".to_string();
    progress(Progress { fraction: 0.0, stage: "画像を読み込み中 / Reading the image" });
    let img = image::load_from_memory(input).map_err(|e| format!("画像を読み込めません(PNG・JPEG に対応) / Cannot read the image (PNG and JPEG are supported): {e}"))?.to_rgb8();
    let (sw, sh) = (img.width(), img.height());
    let (ow, oh) = output_size(sw, sh, opts.target);
    // 拡大が必要なときだけ AI を使う(縮小・ほぼ同じ大きさなら、補間だけ)。
    let needs_up = (ow as f64 / sw as f64).max(oh as f64 / sh as f64) > 1.05;
    let use_ai = opts.ai && needs_up;

    let working: RgbImage = if use_ai {
        let model = load_model()?;
        let (w, h) = (sw as usize, sh as usize);
        let r = model.scale();
        let mut out = vec![0u8; w * r * h * r * 3];
        let mut y = 0;
        while y < h {
            if cancelled() {
                return Err(cancel());
            }
            let y1 = (y + BAND_ROWS).min(h);
            let (ry0, ry1) = (y.saturating_sub(18), (y1 + 18).min(h));
            let region: Vec<f32> = img.as_raw()[ry0 * w * 3..ry1 * w * 3].iter().map(|&b| b as f32 / 255.0).collect();
            let up = model::upscale_rgb_threads(&model, &region, w, ry1 - ry0, opts.threads);
            let row_len = w * r * 3;
            let (a, b) = ((y - ry0) * r, (y1 - ry0) * r);
            for (i, row) in (a..b).enumerate() {
                let dst = (y * r + i) * row_len;
                for (d, s) in out[dst..dst + row_len].iter_mut().zip(&up[row * row_len..(row + 1) * row_len]) {
                    *d = (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                }
            }
            y = y1;
            progress(Progress { fraction: 0.05 + 0.8 * (y as f32 / h as f32), stage: "AI超解像で拡大中 / Enlarging with AI super-resolution" });
        }
        RgbImage::from_raw((w * r) as u32, (h * r) as u32, out).ok_or("出力画像の組み立てに失敗しました / Failed to assemble the output image")?
    } else {
        img
    };

    if cancelled() {
        return Err(cancel());
    }
    progress(Progress { fraction: 0.88, stage: "目標の大きさに合わせて高品質に拡大縮小中 / Resampling to the target size" });
    let resized = if working.dimensions() == (ow, oh) { working } else { image::imageops::resize(&working, ow, oh, image::imageops::FilterType::Lanczos3) };

    progress(Progress { fraction: 0.95, stage: "保存用に変換中 / Encoding" });
    let mut bytes = Vec::new();
    match opts.format {
        Format::Png => image::codecs::png::PngEncoder::new_with_quality(&mut bytes, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Adaptive)
            .write_image(resized.as_raw(), ow, oh, image::ExtendedColorType::Rgb8),
        Format::Jpeg => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92).write_image(resized.as_raw(), ow, oh, image::ExtendedColorType::Rgb8),
    }
    .map_err(|e| format!("画像を保存形式に変換できません / Cannot encode the image: {e}"))?;
    progress(Progress { fraction: 1.0, stage: "完了 / Done" });
    Ok(Output { bytes, width: ow, height: oh, used_ai: use_ai })
}

/// 小さな切り抜き(OCR の行画像など)を `scale` 倍(整数 2〜4)に拡大する。AI 超解像(`ai`)なら 4 倍で作って `scale` へ縮小、
/// そうでなければ高品質な補間(Lanczos3)だけ。文字のかすれ・つぶれを補ってから OCR にかける用途。
pub fn upscale_crop(img: &RgbImage, scale: u32, ai: bool, threads: usize) -> Result<RgbImage, String> {
    let scale = scale.clamp(2, 4);
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err("empty image".to_string());
    }
    if !ai {
        return Ok(image::imageops::resize(img, w * scale, h * scale, image::imageops::FilterType::Lanczos3));
    }
    static MODEL: std::sync::OnceLock<Result<SrModel, String>> = std::sync::OnceLock::new();
    let model = MODEL.get_or_init(load_model).as_ref().map_err(|e| e.clone())?;
    let r = model.scale();
    let rgb: Vec<f32> = img.as_raw().iter().map(|&b| b as f32 / 255.0).collect();
    let up = model::upscale_rgb_threads(model, &rgb, w as usize, h as usize, threads);
    let bytes: Vec<u8> = up.iter().map(|s| (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8).collect();
    let big = RgbImage::from_raw(w * r as u32, h * r as u32, bytes).ok_or("failed to assemble the enlarged image")?;
    if r as u32 == scale {
        Ok(big)
    } else {
        Ok(image::imageops::resize(&big, w * scale, h * scale, image::imageops::FilterType::Lanczos3))
    }
}


/// 画像を読み込まずに、出力の大きさだけを知りたいとき(画面の表示用)。
pub fn dimensions(input: &[u8]) -> Result<(u32, u32), String> {
    let img = image::load_from_memory(input).map_err(|e| e.to_string())?;
    Ok(match &img {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_) | _ => (img.width(), img.height()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_sizes_fit_the_frame_and_keep_the_aspect() {
        assert_eq!(output_size(640, 360, Target::FullHd), (1920, 1080));
        assert_eq!(output_size(640, 480, Target::FullHd), (1440, 1080)); // 4:3 は高さに合わせる
        assert_eq!(output_size(480, 640, Target::FullHd), (1080, 1440)); // 縦長は枠を縦横入れ替える
        assert_eq!(output_size(1920, 1080, Target::Uhd8k), (7680, 4320));
        assert_eq!(output_size(1000, 1000, Target::Uhd4k), (2160, 2160));
        assert_eq!(output_size(4000, 3000, Target::Qhd), (1920, 1440));
        let (w, h) = output_size(3, 2, Target::Uhd16k);
        assert_eq!((w, h), (12960, 8640));
    }

    #[test]
    fn model_loads_and_scales_by_four() {
        let m = load_model().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(m.scale(), 4);
    }

    fn gradient(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, 128]));
        let mut out = Vec::new();
        let _ = image::codecs::png::PngEncoder::new(&mut out).write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8);
        out
    }

    #[test]
    fn resample_only_hits_the_exact_target_size() {
        let src = gradient(64, 36);
        let out = upscale(&src, Options { target: Target::FullHd, ai: false, format: Format::Jpeg, threads: 1 }, &mut |_| {}, &|| false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((out.width, out.height, out.used_ai), (1920, 1080, false));
        let back = image::load_from_memory(&out.bytes).unwrap_or_else(|e| panic!("{e}")).to_rgb8();
        assert_eq!(back.dimensions(), (1920, 1080));
    }

    #[test]
    fn ai_pass_runs_on_a_small_image_and_can_be_cancelled() {
        let src = gradient(48, 27);
        let mut seen = Vec::new();
        let out = upscale(&src, Options { target: Target::FullHd, ai: true, format: Format::Png, threads: 0 }, &mut |p| seen.push(p.fraction), &|| false).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((out.width, out.height, out.used_ai), (1920, 1080, true));
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress must not go backwards: {seen:?}");
        let cancelled = upscale(&src, Options { target: Target::FullHd, ai: true, format: Format::Png, threads: 0 }, &mut |_| {}, &|| true);
        assert!(cancelled.is_err());
    }

    #[test]
    fn shrinking_does_not_use_ai() {
        let src = gradient(400, 225);
        let out = upscale(&src, Options { target: Target::FullHd, ai: true, format: Format::Png, threads: 1 }, &mut |_| {}, &|| false);
        // 400x225 → 1920x1080 は拡大なので AI。縮小になる場合は AI を使わない。
        assert!(out.is_ok());
        let big = gradient(2400, 1350);
        let out = upscale(&big, Options { target: Target::FullHd, ai: true, format: Format::Jpeg, threads: 1 }, &mut |_| {}, &|| false).unwrap_or_else(|e| panic!("{e}"));
        assert!(!out.used_ai && (out.width, out.height) == (1920, 1080));
    }

    #[test]
    fn rejects_non_images() {
        assert!(upscale(b"not an image", Options { target: Target::Uhd4k, ai: false, format: Format::Png, threads: 1 }, &mut |_| {}, &|| false).is_err());
    }
}
