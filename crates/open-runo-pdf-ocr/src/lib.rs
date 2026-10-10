//! PDF 綴じ方向アプリ(`pdf-r2l-*`)の「OCR・テキストフォント文字化」機能の共有ライブラリ。
//!
//! 流れ(`process`): ページを画像にする(純 Rust の `hayro`)→ `aruaru-llm` の `POST /v1/ocr` で
//! 複数ページをまとめて OCR → 行ごとに書体の系統(ゴシック/明朝)・太字・文字色・背景色を推定 →
//! 元の書体に合わせた Noto フォントの文字に置き換え、検索できる文字データとして PDF に書き込む。
//!
//! **オンライン環境が必要**(OCR とフォント・言語データの取得に `aruaru-llm` へ接続する)。
//! 画像は処理後に破棄され、保存されない。OCR・書体の推定には誤りがあり得る。

pub mod client;
pub mod compose;
pub mod fonts;
pub mod model;
pub mod render;
pub mod settings;
pub mod style;

pub use client::{LanguageEntry, LanguageList, OcrClient};
pub use model::{OcrLine, OcrWord, Progress, Quality, Report, Settings, Stage, MAX_LANGUAGES};

use compose::{LinePlan, PagePlan};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// 縦書きらしい行(高さが幅より十分に大きい)か。縦書きは置き換えず、見えない文字だけにする。
fn looks_vertical(line: &OcrLine) -> bool {
    line.text.chars().count() > 1 && (line.h as f32) > (line.w as f32) * 1.5
}

/// PDF を OCR し、`settings.enabled` なら文字を置き換えた PDF を返す。
/// `cache_dir` はフォントのキャッシュ先。`progress` で進捗を受け取る。
pub fn process(
    pdf: &[u8],
    settings: &Settings,
    cache_dir: &Path,
    progress: &mut dyn FnMut(Progress),
) -> Result<(Vec<u8>, Report), String> {
    settings.validate()?;
    let client = OcrClient::new(&settings.server_url);
    let total = render::page_count(pdf)?;
    progress(Progress {
        stage: Stage::Prepare,
        done: 0,
        total,
    });
    client.prepare(&settings.languages, settings.quality)?;

    let mut plans: Vec<PagePlan> = Vec::with_capacity(total);
    let mut start = 0;
    while start < total {
        let end = (start + settings.batch_pages).min(total);
        progress(Progress {
            stage: Stage::Ocr,
            done: start,
            total,
        });
        let rasters = render::render_pages(pdf, start..end, settings.render_long_side_px)?;
        let pngs: Vec<Vec<u8>> = rasters.iter().map(|r| r.png.clone()).collect();
        let pages = client.ocr(&pngs, &settings.languages, settings.quality)?;
        for (raster, lines) in rasters.iter().zip(pages) {
            let lines = lines
                .into_iter()
                .filter(|l| !l.text.trim().is_empty() && l.w > 0 && l.h > 0)
                .map(|line| {
                    let style = style::analyze(&raster.rgba, raster.width, raster.height, line.x, line.y, line.w, line.h).classify(&line.text, line.w);
                    let retype = line.conf >= settings.min_confidence && style.has_ink && !looks_vertical(&line);
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
    let report = compose::compose(&mut doc, &plans, &settings.languages, &mut provider)?;
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
