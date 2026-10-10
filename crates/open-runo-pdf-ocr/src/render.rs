//! PDF のページを画像にする(純 Rust の `hayro`。pdfium などのネイティブライブラリは不要で、
//! Windows / macOS / Linux / Android で同じように動く)。

use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{render, PixmapSettings, RenderCache, RenderSettings};
use std::ops::Range;

/// 画像にした1ページ。
#[derive(Debug, Clone)]
pub struct PageRaster {
    /// 幅(px)。
    pub width: usize,
    /// 高さ(px)。
    pub height: usize,
    /// 不透明な RGBA(白背景)。
    pub rgba: Vec<u8>,
    /// OCR に送る PNG。
    pub png: Vec<u8>,
    /// 画像ピクセル座標 → PDF ページのユーザー空間への変換 `[a b c d e f]`(PDF の行列と同じ並び)。
    pub px_to_user: [f64; 6],
}

/// PDF のページ数。
pub fn page_count(pdf: &[u8]) -> Result<usize, String> {
    let doc = Pdf::new(pdf.to_vec()).map_err(|e| format!("PDFを読み込めません(暗号化・破損の可能性) / Cannot read the PDF (encrypted or damaged?): {e:?}"))?;
    Ok(doc.pages().len())
}

/// `range` のページを画像にする。長辺は `long_side_px` 以下(最大 300dpi 相当)。
pub fn render_pages(
    pdf: &[u8],
    range: Range<usize>,
    long_side_px: u32,
) -> Result<Vec<PageRaster>, String> {
    let doc = Pdf::new(pdf.to_vec())
        .map_err(|e| format!("PDFを読み込めません / Cannot read the PDF: {e:?}"))?;
    let settings = InterpreterSettings::default();
    let cache = RenderCache::new();
    let mut out = Vec::new();
    for (idx, page) in doc.pages().iter().enumerate() {
        if idx < range.start {
            continue;
        }
        if idx >= range.end {
            break;
        }
        let (w_pt, h_pt) = page.render_dimensions();
        let long = w_pt.max(h_pt).max(1.0);
        let scale = (300.0f32 / 72.0).min(long_side_px as f32 / long).max(0.1);
        let pixmap = render(
            page,
            &cache,
            &settings,
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
            },
        );
        let width = pixmap.width() as usize;
        let height = pixmap.height() as usize;
        let rgba = pixmap.data_as_u8_slice().to_vec();
        let png = pixmap
            .clone()
            .into_png()
            .map_err(|e| format!("PNG encode: {e}"))?;
        let user_to_pt: Affine = page.initial_transform(true).to_kurbo();
        let px_to_user = user_to_pt.inverse() * Affine::scale(1.0 / scale as f64);
        out.push(PageRaster {
            width,
            height,
            rgba,
            png,
            px_to_user: px_to_user.as_coeffs(),
        });
    }
    Ok(out)
}

impl PageRaster {
    /// ほぼ白紙(文字・図がほとんど無い)か。白紙のページは OCR に送らず、時間と通信を節約する。
    pub fn is_blank(&self) -> bool {
        let step = 3usize;
        let mut hist = [0u32; 256];
        let mut n = 0u32;
        let mut y = 0;
        while y < self.height {
            let mut x = 0;
            while x < self.width {
                let i = (y * self.width + x) * 4;
                let l = (299 * self.rgba[i] as u32
                    + 587 * self.rgba[i + 1] as u32
                    + 114 * self.rgba[i + 2] as u32)
                    / 1000;
                hist[l.min(255) as usize] += 1;
                n += 1;
                x += step;
            }
            y += step;
        }
        if n == 0 {
            return true;
        }
        // 背景(中央値)より 60 以上暗い画素が 0.02% 未満なら白紙。
        let mut acc = 0u32;
        let mut median = 255usize;
        for (l, &c) in hist.iter().enumerate() {
            acc += c;
            if acc * 2 >= n {
                median = l;
                break;
            }
        }
        let dark: u32 = hist.iter().take(median.saturating_sub(60)).sum();
        (dark as f32) < n as f32 * 0.0002
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(rgba: Vec<u8>, w: usize, h: usize) -> PageRaster {
        PageRaster {
            width: w,
            height: h,
            rgba,
            png: Vec::new(),
            px_to_user: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    #[test]
    fn blank_and_inked_pages() {
        let (w, h) = (200usize, 200usize);
        assert!(raster(vec![255; w * h * 4], w, h).is_blank());
        let mut v = vec![255u8; w * h * 4];
        for y in 50..60 {
            for x in 20..180 {
                let i = (y * w + x) * 4;
                v[i] = 0;
                v[i + 1] = 0;
                v[i + 2] = 0;
            }
        }
        assert!(!raster(v, w, h).is_blank());
    }
}
