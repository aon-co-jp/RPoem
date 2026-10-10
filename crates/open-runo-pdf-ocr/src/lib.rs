//! PDF 綴じ方向アプリ(`pdf-r2l-*`)の「OCR・テキストフォント文字化」機能の共有ライブラリ。
//!
//! 流れ(`process`): ページを画像にする(純 Rust の `hayro`)→ `aruaru-llm` の `POST /v1/ocr` で
//! 複数ページをまとめて OCR → 行ごとに書体の系統(ゴシック/明朝)・太字・文字色・背景色を推定 →
//! 元の書体に合わせた Noto フォントの文字に置き換え、検索できる文字データとして PDF に書き込む。
//!
//! **オンライン環境が必要**(OCR とフォント・言語データの取得に `aruaru-llm` へ接続する)。
//! 画像は処理後に破棄され、保存されない。OCR・書体の推定には誤りがあり得る。

pub mod client;
pub mod complete;
pub mod compose;
pub mod enhance;
pub mod fonts;
pub mod languages;
pub mod model;
pub mod render;
pub mod settings;
pub mod style;
pub mod visual;

pub use client::{LanguageEntry, LanguageList, OcrClient};
pub use model::{OcrLine, OcrWord, Progress, Quality, Report, Settings, Stage, MAX_LANGUAGES};

use compose::{LinePlan, PagePlan};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// 文字に置き換えてよいほど信頼できる行か。行の平均の信頼度がしきい値以上で、かつ単語の 8 割以上が信頼度 50 以上。
/// (読める元の画像を、誤認識した文字で置き換えてしまう被害を減らす。言語の選び間違いなどで平均が高くても、単語ごとに見れば崩れる。)
fn line_is_reliable(line: &OcrLine, min_confidence: f32) -> bool {
    if line.conf < min_confidence {
        return false;
    }
    let words: Vec<_> = line
        .words
        .iter()
        .filter(|w| !w.text.trim().is_empty())
        .collect();
    if words.is_empty() {
        return false;
    }
    let weak = words.iter().filter(|w| w.conf < 50.0).count();
    weak * 5 <= words.len()
}

/// 縦書きらしい行(高さが幅より十分に大きい)か。縦書きは置き換えず、見えない文字だけにする。
fn looks_vertical(line: &OcrLine) -> bool {
    line.text.chars().count() > 1 && (line.h as f32) > (line.w as f32) * 1.5
}

/// PDF を OCR し、`settings.enabled` なら文字を置き換えた PDF を返す。
/// `cache_dir` はフォントのキャッシュ先。`progress` で進捗を受け取る。
///
/// 流れ(バッチ = 数ページごと): ページを画像にする → 薄いページはコントラストを自動補正 → OCR →
/// (設定で)かすれ補正(自信の低い行を超解像で拡大して再 OCR)→(設定で)AI 文脈補完 → 書体の判定と文字化。
pub fn process(
    pdf: &[u8],
    settings: &Settings,
    cache_dir: &Path,
    progress: &mut dyn FnMut(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Report), String> {
    let cancel_err = || "キャンセルしました / Cancelled".to_string();
    settings.validate()?;
    let client = OcrClient::new(&settings.server_url);
    let total = render::page_count(pdf)?;
    progress(Progress {
        stage: Stage::Prepare,
        done: 0,
        total,
    });
    client.prepare(&settings.languages, settings.quality)?;

    // 字形の照合は、サーバーが対応していればサーバー(`open-cuda` の sgemm)で行う。アプリ内で計算するときだけ、フォントを取得する(遅延)。
    let load_glyphs = || -> Option<open_runo_glyph::Glyphs> {
        let sans = client.font("NotoSansJP-VF.ttf", cache_dir).ok();
        let serif = client.font("NotoSerifJP-VF.ttf", cache_dir).ok();
        let fonts: Vec<&[u8]> = [serif.as_deref(), sans.as_deref()].into_iter().flatten().collect();
        open_runo_glyph::Glyphs::with_fonts(&fonts)
    };

    let mut plans: Vec<PagePlan> = Vec::with_capacity(total);
    let mut filled = 0usize;
    let mut reread = 0usize;
    let mut start = 0;
    while start < total {
        if cancelled() {
            return Err(cancel_err());
        }
        let end = (start + settings.batch_pages).min(total);
        progress(Progress {
            stage: Stage::Ocr,
            done: start,
            total,
        });
        let t0 = std::time::Instant::now();
        // 薄い(低コントラストの)ページは、濃淡を引き伸ばす(これが無いと、薄い文書は「白紙」と判定されて OCR にすら送られない)。
        let rasters: Vec<render::PageRaster> = render::render_pages(pdf, start..end, settings.render_long_side_px)?
            .into_iter()
            .map(|r| enhance::normalize_contrast(&r).unwrap_or(r))
            .collect();
        let t1 = std::time::Instant::now();
        // 白紙のページは OCR に送らない。
        let send: Vec<usize> = (0..rasters.len())
            .filter(|&i| !rasters[i].is_blank())
            .collect();
        let pngs: Vec<Vec<u8>> = send.iter().map(|&i| rasters[i].png.clone()).collect();
        let mut sent_pages = if pngs.is_empty() {
            Vec::new()
        } else {
            client.ocr(&pngs, &settings.languages, settings.quality)?
        }
        .into_iter();
        let mut pages: Vec<Vec<OcrLine>> = vec![Vec::new(); rasters.len()];
        for &i in &send {
            pages[i] = sent_pages.next().unwrap_or_default();
        }
        if std::env::var_os("PDFOCR_DEBUG").is_some() {
            eprintln!(
                "[time] pages {start}..{end}: render {:.1}s, ocr {:.1}s, png {} KB total",
                (t1 - t0).as_secs_f32(),
                t1.elapsed().as_secs_f32(),
                pngs.iter().map(|p| p.len()).sum::<usize>() / 1024
            );
        }
        // かすれ補正(設定でオンのときだけ): 自信の低い行を、ノイズ除去 → 拡大(AI 超解像 / 補間)→ 再 OCR で読み直す。
        if settings.reocr {
            for (raster, lines) in rasters.iter().zip(pages.iter_mut()) {
                match enhance::reocr_lines(&client, raster, lines, settings) {
                    Ok(n) => reread += n,
                    Err(e) => {
                        if std::env::var_os("PDFOCR_DEBUG").is_some() {
                            eprintln!("[reocr] failed: {e}");
                        }
                    }
                }
            }
        }
        // AI 文脈補完(設定でオンのときだけ)。読み・欠けを直したあとの文字列で、書体判定と文字化を行う。
        filled += complete::run(&client, &mut pages, &rasters, settings, &load_glyphs);
        for (raster, lines) in rasters.iter().zip(pages) {
            let lines = lines
                .into_iter()
                .filter(|l| !l.text.trim().is_empty() && l.w > 0 && l.h > 0)
                .map(|line| {
                    let style = style::analyze(&raster.rgba, raster.width, raster.height, line.x, line.y, line.w, line.h).classify(&line.text, line.w);
                    let retype = settings.replace_text
                        && line_is_reliable(&line, settings.min_confidence)
                        && style.has_ink
                        && !looks_vertical(&line);
                    if std::env::var_os("PDFOCR_DEBUG").is_some() {
                        eprintln!("[ocr] conf={:.0} serif={} bold={} contrast={:.2} stem={:.3} retype={} | {}", line.conf, style.serif, style.bold, style.contrast, style.stem_ratio, retype, line.text);
                    }
                    LinePlan { line, style, retype }
                })
                .collect();
            plans.push(PagePlan {
                px_to_user: raster.px_to_user,
                lines,
            });
        }
        start = end;
    }
    if cancelled() {
        return Err(cancel_err());
    }
    progress(Progress {
        stage: Stage::Compose,
        done: 0,
        total,
    });

    let mut doc = lopdf::Document::load_mem(pdf).map_err(|e| {
        format!("PDFを読み込めません(暗号化・破損の可能性) / Cannot read the PDF: {e}")
    })?;
    let mut font_cache: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    let mut provider = |name: &str| -> Result<Arc<Vec<u8>>, String> {
        if let Some(f) = font_cache.get(name) {
            return Ok(Arc::clone(f));
        }
        let bytes = Arc::new(client.font(name, cache_dir)?);
        font_cache.insert(name.to_string(), Arc::clone(&bytes));
        Ok(bytes)
    };
    let mut report = compose::compose(&mut doc, &plans, &settings.languages, &mut provider)?;
    report.chars_filled = filled;
    report.lines_reread = reread;
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("保存失敗 / Save failed: {e}"))?;
    progress(Progress {
        stage: Stage::Compose,
        done: total,
        total,
    });
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(conf: f32) -> OcrWord {
        OcrWord {
            text: "w".into(),
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            conf,
        }
    }

    fn line(conf: f32, word_confs: &[f32]) -> OcrLine {
        OcrLine {
            text: "w w w w w".into(),
            x: 0,
            y: 0,
            w: 50,
            h: 10,
            conf,
            words: word_confs.iter().map(|&c| word(c)).collect(),
        }
    }

    #[test]
    fn reliability_needs_line_and_word_confidence() {
        assert!(line_is_reliable(
            &line(90.0, &[90.0, 92.0, 88.0, 95.0, 91.0]),
            70.0
        ));
        assert!(
            !line_is_reliable(&line(60.0, &[60.0; 5]), 70.0),
            "below the line threshold"
        );
        assert!(
            !line_is_reliable(&line(80.0, &[95.0, 95.0, 95.0, 30.0, 20.0]), 70.0),
            "too many weak words"
        );
        assert!(
            line_is_reliable(&line(85.0, &[95.0, 95.0, 95.0, 95.0, 30.0]), 70.0),
            "one weak word of five is tolerated"
        );
    }
}
