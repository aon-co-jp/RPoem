//! AI 文脈補完: OCR の自信が低い・欠けた・読み違えた **1〜2文字** を、前後の長い文脈から言語モデルで直す。
//!
//! 長文を書き換えるのではない。疑わしい箇所だけを `(前の文脈, 疑わしい文字列, 後ろの文脈)` にして、サーバー
//! (`aruaru-llm` の `POST /v1/ocr/fill`)へ送り、言語モデルが候補を採点して、十分に尤もらしいときだけ置き換える。
//!
//! 疑わしい箇所の見つけ方(両方使う):
//! 1. OCR の信頼度が低い短い単語、文字がまるごと欠けた疑いのある隙間(CJK)。
//! 2. **言語モデルによる走査**(`POST /v1/ocr/scan`): 各行をモデルに読ませ、文脈にとって不自然(確率が極端に低い)な
//!    トークンを挙げさせる。「毎朝」→「耕朝」のように、自信満々に読み違えた箇所も見つかる。
//!
//! 置き換えは 1 か所・最大 2 文字で、元の読みより明確に尤もらしいときに限る。サーバーに言語モデルが無いとき(503)は何も変えない。

use crate::model::{OcrLine, Settings};
use serde::{Deserialize, Serialize};

/// サーバーへ送る 1 か所。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FillItem {
    pub prefix: String,
    pub suspect: String,
    pub suffix: String,
    pub gap: bool,
    /// OCR の信頼度(0〜100、不明なら -1)。
    pub conf: f32,
    /// 字形の照合で選んだ追加の候補(疑わしい文字列を置き換える完全な文字列)。
    pub extra: Vec<String>,
}

/// 採点された候補 1 つ。
#[derive(Debug, Clone, Deserialize)]
pub struct Scored {
    pub text: String,
    pub logp: f32,
}

/// サーバーからの結果 1 か所。
#[derive(Debug, Clone, Deserialize)]
pub struct FillResult {
    pub index: usize,
    pub chosen: String,
    pub changed: bool,
    /// 元の読みの対数尤度。
    #[serde(default)]
    pub orig_logp: f32,
    /// 採点した全候補(`return_all` のとき)。
    #[serde(default)]
    pub all: Vec<Scored>,
}

/// 候補の提案 1 つ(疑わしい文字列の `pos` 文字目に入り得る 1 文字)。
#[derive(Debug, Clone, Deserialize)]
pub struct Proposal {
    pub pos: usize,
    pub text: String,
    #[serde(default)]
    pub lm: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProposeResult {
    pub index: usize,
    pub proposals: Vec<Proposal>,
}

/// 走査の依頼 1 行 / 結果。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanItem {
    pub prefix: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanSpot {
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScanResult {
    pub index: usize,
    pub spots: Vec<ScanSpot>,
}

/// 行の中の位置(文字単位)。
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub page: usize,
    pub line: usize,
    /// 行の文字列(`char` 単位)での開始位置。
    pub start: usize,
    /// 置き換える長さ(`gap` のときは 0)。
    pub len: usize,
    /// 行の単語の番号。置き換え・削除では疑わしい文字を含む単語、欠けた文字の補充では直前の単語。
    pub word: usize,
    /// その単語の文字列の中での位置(補充では単語の末尾 = 単語の長さ)。
    pub woff: usize,
}

const CONTEXT_PREV: usize = 40;
const CONTEXT_NEXT: usize = 24;

fn tail(s: &str, n: usize) -> String {
    let v: Vec<char> = s.chars().collect();
    v[v.len().saturating_sub(n)..].iter().collect()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xFF00..=0xFFEF)
}

/// 行の文字列の中での、各単語の開始位置(`char` 単位)。見つからない単語は `None`。
fn word_starts(line: &OcrLine) -> Vec<Option<usize>> {
    let chars: Vec<char> = line.text.chars().collect();
    let mut cursor = 0usize;
    let mut out = Vec::with_capacity(line.words.len());
    for w in &line.words {
        let wchars: Vec<char> = w.text.chars().collect();
        if wchars.is_empty() || wchars.len() > chars.len() {
            out.push(None);
            continue;
        }
        let found = (cursor..=chars.len() - wchars.len())
            .find(|&i| chars[i..i + wchars.len()] == wchars[..]);
        if let Some(s) = found {
            cursor = s + wchars.len();
        }
        out.push(found);
    }
    out
}

/// 前の行の末尾・次の行の先頭を足した文脈。
fn context(lines: &[OcrLine], li: usize, start: usize, len: usize) -> (String, String) {
    let line = &lines[li];
    let chars: Vec<char> = line.text.chars().collect();
    let join = if line.text.chars().any(is_cjk) {
        ""
    } else {
        " "
    };
    let before = if li > 0 {
        format!("{}{join}", tail(&lines[li - 1].text, CONTEXT_PREV))
    } else {
        String::new()
    };
    let after_line: String = lines
        .get(li + 1)
        .map(|l| l.text.chars().take(CONTEXT_NEXT).collect())
        .unwrap_or_default();
    let pre: String = chars[..start.min(chars.len())].iter().collect();
    let suf: String = chars[(start + len).min(chars.len())..].iter().collect();
    (
        format!("{before}{pre}"),
        format!(
            "{suf}{}{after_line}",
            if after_line.is_empty() { "" } else { join }
        ),
    )
}

/// OCR の信頼度と隙間から、疑わしい箇所を集める。`below` 未満の信頼度の短い単語と、CJK の単語の間の不自然な隙間(最大 `max_spots` か所)。
pub fn collect(pages: &[Vec<OcrLine>], below: f32, max_spots: usize) -> Vec<(Spot, FillItem)> {
    collect_with(pages, below, max_spots, true)
}

/// `use_conf` が偽なら、信頼度の低い単語は対象にせず、隙間(欠けた文字)だけを集める。
pub fn collect_with(
    pages: &[Vec<OcrLine>],
    below: f32,
    max_spots: usize,
    use_conf: bool,
) -> Vec<(Spot, FillItem)> {
    let mut out = Vec::new();
    for (pi, lines) in pages.iter().enumerate() {
        for (li, line) in lines.iter().enumerate() {
            let chars: Vec<char> = line.text.chars().collect();
            let starts = word_starts(line);
            // 1 文字あたりの幅(CJK の隙間の判定用)。
            let char_w = {
                let mut ws: Vec<f32> = line
                    .words
                    .iter()
                    .filter(|w| !w.text.is_empty())
                    .map(|w| w.w as f32 / w.text.chars().count() as f32)
                    .collect();
                ws.sort_by(|a, b| a.total_cmp(b));
                ws.get(ws.len() / 2).copied().unwrap_or(0.0)
            };
            // 1 文字ずつの単語(再 OCR の結果など)では、隙間の判定が当てにならない。
            let wide_words = line
                .words
                .iter()
                .map(|w| w.text.chars().count())
                .sum::<usize>()
                >= line.words.len() * 2;
            let mut prev_end: Option<(usize, i32, usize)> = None; // (直前の単語の終わりの位置, 右端 px, 単語の番号)
            for (wi, w) in line.words.iter().enumerate() {
                let wlen = w.text.chars().count();
                let Some(start) = starts[wi] else {
                    continue;
                };
                // 欠けた文字の疑い: 直前の単語との隙間が、1 文字ぶんの幅の 0.8 倍以上、かつ文字列の上で隣り合っている CJK。
                if let Some((end_pos, right_px, prev_wi)) = prev_end {
                    let gap = w.x - right_px;
                    if end_pos == start
                        && char_w > 0.0
                        && gap as f32 >= char_w * 1.0
                        && wide_words
                        && start > 0
                        && is_cjk(chars[start - 1])
                        && is_cjk(chars[start])
                        && out.len() < max_spots
                    {
                        let (prefix, suffix) = context(lines, li, start, 0);
                        let pw = line.words[prev_wi].text.chars().count();
                        out.push((
                            Spot {
                                page: pi,
                                line: li,
                                start,
                                len: 0,
                                word: prev_wi,
                                woff: pw,
                            },
                            FillItem {
                                prefix,
                                suspect: String::new(),
                                suffix,
                                gap: true,
                                conf: -1.0,
                                extra: Vec::new(),
                            },
                        ));
                    }
                }
                prev_end = Some((start + wlen, w.x + w.w, wi));
                if use_conf
                    && w.conf >= 0.0
                    && w.conf < below
                    && wlen <= 8
                    && out.len() < max_spots
                    && w.text.chars().any(|c| c.is_alphanumeric())
                {
                    let (prefix, suffix) = context(lines, li, start, wlen);
                    out.push((
                        Spot {
                            page: pi,
                            line: li,
                            start,
                            len: wlen,
                            word: wi,
                            woff: 0,
                        },
                        FillItem {
                            prefix,
                            suspect: w.text.clone(),
                            suffix,
                            gap: false,
                            conf: w.conf,
                            extra: Vec::new(),
                        },
                    ));
                }
            }
            if out.len() >= max_spots {
                return out;
            }
        }
    }
    out
}

/// 走査の依頼(全行、前の行を文脈にして)。
pub fn scan_items(pages: &[Vec<OcrLine>]) -> Vec<(usize, usize, ScanItem)> {
    let mut v = Vec::new();
    for (pi, lines) in pages.iter().enumerate() {
        for (li, line) in lines.iter().enumerate() {
            if line.text.chars().count() < 3 {
                continue;
            }
            let join = if line.text.chars().any(is_cjk) {
                ""
            } else {
                " "
            };
            let prefix = if li > 0 {
                format!("{}{join}", tail(&lines[li - 1].text, CONTEXT_PREV))
            } else {
                String::new()
            };
            v.push((
                pi,
                li,
                ScanItem {
                    prefix,
                    text: line.text.clone(),
                },
            ));
        }
    }
    v
}

/// 走査の結果から、疑わしい箇所を作る。すでにある箇所と重なるもの、単語をまたぐものは除く。
pub fn spots_from_scan(
    pages: &[Vec<OcrLine>],
    asked: &[(usize, usize, ScanItem)],
    results: &[ScanResult],
    existing: &[(Spot, FillItem)],
    max_total: usize,
) -> Vec<(Spot, FillItem)> {
    let mut out: Vec<(Spot, FillItem)> = Vec::new();
    for r in results {
        let Some(&(pi, li, _)) = asked.get(r.index) else {
            continue;
        };
        let lines = &pages[pi];
        let line = &lines[li];
        let starts = word_starts(line);
        for s in &r.spots {
            if existing.len() + out.len() >= max_total {
                return out;
            }
            let overlaps = existing.iter().any(|(e, _)| {
                e.page == pi
                    && e.line == li
                    && s.start < e.start + e.len.max(1)
                    && e.start < s.start + s.len
            });
            if overlaps || s.len == 0 {
                continue;
            }
            // 範囲が単語にまたがるときは、置き換えの際に単語を 1 つにまとめる(`apply`)。範囲の先頭が属する単語を記録しておく。
            let found = line.words.iter().enumerate().find_map(|(wi, w)| {
                let ws = starts[wi]?;
                let we = ws + w.text.chars().count();
                (s.start >= ws && s.start < we).then(|| (wi, s.start - ws))
            });
            let Some((wi, woff)) = found else {
                continue;
            };
            let (prefix, suffix) = context(lines, li, s.start, s.len);
            let suspect: String = line.text.chars().skip(s.start).take(s.len).collect();
            // 走査由来の箇所は、削除の候補にしない(conf = -1): 不自然な字を消してしまうより、別の字に直すことだけを考える。
            out.push((
                Spot {
                    page: pi,
                    line: li,
                    start: s.start,
                    len: s.len,
                    word: wi,
                    woff,
                },
                FillItem {
                    prefix,
                    suspect,
                    suffix,
                    gap: false,
                    conf: -1.0,
                    extra: Vec::new(),
                },
            ));
        }
    }
    out
}

/// 採用された置き換えを行の文字列と単語へ反映する(後ろの位置から順に、位置がずれないように)。戻り値は置き換えた数。
/// 置き換える範囲が複数の単語にまたがるとき(再 OCR の 1 文字ずつの単語など)は、その単語たちを 1 つにまとめてから直す。
/// 描画・検索用の文字は単語から作られるため、行の文字列と単語の両方を直す。
pub fn apply(
    pages: &mut [Vec<OcrLine>],
    spots: &[(Spot, FillItem)],
    results: &[FillResult],
) -> usize {
    let mut chosen: Vec<(&Spot, &str)> = results
        .iter()
        .filter(|r| r.changed)
        .filter_map(|r| spots.get(r.index).map(|(s, _)| (s, r.chosen.as_str())))
        .collect();
    chosen.sort_by_key(|b| std::cmp::Reverse((b.0.page, b.0.line, b.0.start)));
    let mut n = 0;
    for (spot, text) in chosen {
        let Some(line) = pages.get_mut(spot.page).and_then(|p| p.get_mut(spot.line)) else {
            continue;
        };
        let mut chars: Vec<char> = line.text.chars().collect();
        let (start, len) = (spot.start, spot.len);
        if start + len > chars.len() || text.chars().count() > len + 4 {
            continue;
        }
        let starts = word_starts(line);
        let span = |wi: usize| starts[wi].map(|ws| (ws, ws + line.words[wi].text.chars().count()));
        // 範囲と重なる単語。欠けた文字の補充(len == 0)は、直前に終わる単語(なければ直後に始まる単語)。
        let mut idxs: Vec<usize> = (0..line.words.len())
            .filter(|&wi| {
                span(wi).is_some_and(|(ws, we)| {
                    if len == 0 {
                        we == start
                    } else {
                        ws < start + len && we > start
                    }
                })
            })
            .collect();
        if idxs.is_empty() && len == 0 {
            idxs = (0..line.words.len())
                .filter(|&wi| span(wi).is_some_and(|(ws, _)| ws == start))
                .take(1)
                .collect();
        }
        let (Some(&first), Some(&last)) = (idxs.first(), idxs.last()) else {
            continue;
        };
        let (ws0, _) = span(first).unwrap_or((start, start));
        let (_, we1) = span(last).unwrap_or((start, start));
        let (ws0, we1) = (ws0.min(start), we1.max(start + len));
        if ws0 > start || we1 > chars.len() {
            continue;
        }
        // まとめた単語の文字列(行の文字列の、単語たちが覆う範囲)に、置き換えを適用する。
        let mut merged: Vec<char> = chars[ws0..we1].to_vec();
        merged.splice(start - ws0..start - ws0 + len, text.chars());
        let (mut x0, mut y0, mut x1, mut y1, mut conf) =
            (i32::MAX, i32::MAX, i32::MIN, i32::MIN, 100.0f32);
        for &wi in &idxs {
            let w = &line.words[wi];
            x0 = x0.min(w.x);
            y0 = y0.min(w.y);
            x1 = x1.max(w.x + w.w);
            y1 = y1.max(w.y + w.h);
            conf = conf.min(w.conf);
        }
        let merged_text: String = merged.into_iter().collect();
        line.words.drain(first..=last);
        if !merged_text.is_empty() {
            line.words.insert(
                first,
                crate::model::OcrWord {
                    text: merged_text,
                    x: x0,
                    y: y0,
                    w: x1 - x0,
                    h: y1 - y0,
                    conf,
                },
            );
        }
        chars.splice(start..start + len, text.chars());
        line.text = chars.into_iter().collect();
        n += 1;
    }
    n
}

/// 視覚の重み: 字形の類似度の差 1.0 が、対数尤度の何(nat)にあたるか。
const VISUAL_WEIGHT: f32 = 16.0;

/// 文字の幅の目安(全角 = 1、半角の英数字 = 0.55、半角の記号・空白 = 0.4)。
fn char_weight(c: char) -> f32 {
    if (c as u32) < 0x80 {
        if c.is_ascii_alphanumeric() {
            0.55
        } else {
            0.4
        }
    } else {
        1.0
    }
}

/// 疑わしい文字列の `pos` 文字目の升目(ページ画像の切り抜き)。行の枠を、文字の幅の目安で按分して求める
/// (単語の枠は、再 OCR や CJK の連続した単語では当てにならないため)。
fn cell_for(
    pages: &[Vec<OcrLine>],
    rasters: &[crate::render::PageRaster],
    spot: &Spot,
    pos: usize,
) -> Option<crate::visual::Cell> {
    let line = pages.get(spot.page)?.get(spot.line)?;
    let weights: Vec<f32> = line.text.chars().map(char_weight).collect();
    let total: f32 = weights.iter().sum();
    let idx = spot.start + pos;
    if total <= 0.0 || idx >= weights.len() || line.w <= 0 {
        return None;
    }
    let pitch = line.w as f32 / total; // 全角 1 文字ぶんの幅
    let before: f32 = weights[..idx].iter().sum();
    let cx = line.x as f32 + (before + weights[idx] / 2.0) * pitch;
    let cy = line.y as f32 + line.h as f32 / 2.0;
    crate::visual::cell(
        rasters.get(spot.page)?,
        cx,
        cy,
        (pitch * weights[idx].max(0.55)).max(pitch * 0.9).max(8.0) * 2.0,
    )
}

/// 字形の照合の窓口。サーバー(`aruaru-llm` の sgemm)に任せるか、アプリ内の CPU 参照実装で計算する。
/// どちらも同じ計算(`open-runo-glyph`)なので、結果は同じ(浮動小数点の誤差を除く)。
struct Matcher<'a> {
    target: &'a crate::client::OcrClient,
    /// サーバーが字形の照合に対応しているか(最初の呼び出しで分かる)。
    remote: bool,
    /// アプリ内の計算に使う、字形の行列(必要になったときに作る)。
    local: Option<open_runo_glyph::GlyphIndex>,
    /// アプリ内の計算に使うフォントの読み込み(必要になったときだけ呼ぶ)。
    load_glyphs: &'a dyn Fn() -> Option<open_runo_glyph::Glyphs>,
    pool: Vec<char>,
    extra_chars: String,
}

impl Matcher<'_> {
    /// 升目ごとに、上位 `top` 個の文字と、`query` の文字の類似度を返す。
    fn rank(
        &mut self,
        cells: &[&[f32]],
        query: &[String],
        top: usize,
    ) -> Option<Vec<(CharScores, CharScores)>> {
        if cells.is_empty() {
            return Some(Vec::new());
        }
        if self.remote {
            let maps: Vec<Vec<f32>> = cells.iter().map(|c| c.to_vec()).collect();
            match self.target.glyph_rank(&maps, query, &self.extra_chars, top) {
                Ok(r) => {
                    let conv = |v: Vec<(String, f32)>| {
                        v.into_iter()
                            .filter_map(|(s, x)| s.chars().next().map(|c| (c, x)))
                            .collect::<Vec<_>>()
                    };
                    return Some(
                        r.into_iter()
                            .map(|r| (conv(r.top), conv(r.query)))
                            .collect(),
                    );
                },
                Err(_) => self.remote = false,
            }
        }
        if self.local.is_none() {
            let g = (self.load_glyphs)()?;
            self.local = Some(open_runo_glyph::GlyphIndex::build(&g, &self.pool));
        }
        let index = self.local.as_ref()?;
        Some(
            cells
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let q: Vec<char> = query
                        .get(i)
                        .map(|s| s.chars().collect())
                        .unwrap_or_default();
                    let (t, qv) = index.rank_cpu(c, top, &q);
                    (t.into_iter().map(|(v, c)| (c, v)).collect(), qv)
                })
                .collect(),
        )
    }
}

/// 設定に従って補完する(ページ全体の OCR 結果に対して)。戻り値は置き換えた数。サーバーが使えないときは 0(何も変えない)。
/// `rasters` があれば、残っているインクの形(字形)も照合して、言語モデルの採点と合わせて決める
/// (字形の計算は、サーバーが対応していればサーバー、そうでなければ `load_glyphs` のフォントでアプリ内で行う)。
pub fn run(
    client: &crate::client::OcrClient,
    pages: &mut [Vec<OcrLine>],
    rasters: &[crate::render::PageRaster],
    settings: &Settings,
    load_glyphs: &dyn Fn() -> Option<open_runo_glyph::Glyphs>,
) -> usize {
    if !settings.fill {
        return 0;
    }
    let debug = std::env::var_os("PDFOCR_DEBUG").is_some();
    let other;
    let target = if settings.fill_server_url.trim().is_empty() {
        client
    } else {
        other = crate::client::OcrClient::new(settings.fill_server_url.trim());
        &other
    };
    // 言語モデルによる走査(自信満々の読み違いも見つけられる)を主にする。成功したら、OCR の信頼度ルールは使わず、
    // 欠けた文字の疑い(隙間)だけを足す。走査が使えないとき(サーバーに言語モデルが無いなど)は、信頼度ルールで続ける。
    let asked = scan_items(pages);
    let mut scanned_ok = false;
    let mut scan_results: Vec<ScanResult> = Vec::new();
    if !asked.is_empty() {
        let items: Vec<ScanItem> = asked.iter().map(|(_, _, i)| i.clone()).collect();
        match target.scan(&items) {
            Ok(results) => {
                scanned_ok = true;
                scan_results = results;
            },
            Err(e) => {
                if debug {
                    eprintln!("[fill] scan failed: {e}");
                }
            },
        }
    }
    let mut spots = collect_with(
        pages,
        settings.fill_below,
        settings.fill_max_spots,
        !scanned_ok,
    );
    if scanned_ok {
        let more = spots_from_scan(
            pages,
            &asked,
            &scan_results,
            &spots,
            settings.fill_max_spots,
        );
        spots.extend(more);
    }
    if spots.is_empty() {
        return 0;
    }
    let mut items: Vec<FillItem> = spots.iter().map(|(_, i)| i.clone()).collect();

    // ---- 字形の証拠 ----
    // 升目: 疑わしい文字列の各文字の位置。インクの証拠が弱い(ほぼ消えている)升目は、字形を使わない。
    let use_visual = settings.fill_visual && !rasters.is_empty();
    let cells: Vec<Vec<Option<crate::visual::Cell>>> = if use_visual {
        spots
            .iter()
            .map(|(sp, it)| {
                (0..it.suspect.chars().count())
                    .map(|p| cell_for(pages, rasters, sp, p))
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut doc_chars = String::new();
    for l in pages.iter().flatten() {
        for c in l.text.chars() {
            if c.is_alphanumeric() && !doc_chars.contains(c) {
                doc_chars.push(c);
            }
        }
    }
    let mut matcher = Matcher {
        target,
        remote: use_visual,
        local: None,
        load_glyphs,
        pool: Vec::new(),
        extra_chars: doc_chars.clone(),
    };
    if use_visual {
        // アプリ内で計算する場合の母集団: 頻出の漢字・かな(サーバーの語彙から)+ この文書に出てくる字。
        let mut pool: Vec<char> = target.charset(8000).unwrap_or_default();
        for c in doc_chars.chars() {
            if !pool.contains(&c) {
                pool.push(c);
            }
        }
        matcher.pool = pool;
    }
    // 類似度の表: (箇所, 位置, 文字) → 升目との類似度。
    let mut sims: std::collections::HashMap<(usize, usize, char), f32> =
        std::collections::HashMap::new();
    if use_visual {
        let mut refs: Vec<(usize, usize)> = Vec::new();
        for (idx, cs) in cells.iter().enumerate() {
            for (pos, c) in cs.iter().enumerate() {
                if c.as_ref().is_some_and(|c| c.evidence >= 0.2) {
                    refs.push((idx, pos));
                }
            }
        }
        let maps: Vec<&[f32]> = refs
            .iter()
            .filter_map(|&(i, p)| cells[i][p].as_ref().map(|c| &c.map[..]))
            .collect();
        let queries: Vec<String> = refs
            .iter()
            .map(|&(i, p)| {
                items[i]
                    .suspect
                    .chars()
                    .nth(p)
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            })
            .collect();
        if let Some(ranked) = matcher.rank(&maps, &queries, 24) {
            // 位置ごとの字形の上位候補を、追加の候補(`extra`)にする。連続した複数文字は、位置ごとの上位の組み合わせも加える。
            let mut per_item: Vec<Vec<(usize, CharScores)>> = vec![Vec::new(); items.len()];
            for (&(idx, pos), (top, q)) in refs.iter().zip(ranked) {
                if let Some(&(c, v)) = q.first() {
                    sims.insert((idx, pos, c), v);
                }
                for &(c, v) in &top {
                    sims.insert((idx, pos, c), v);
                }
                per_item[idx].push((pos, top));
            }
            for (idx, per_pos) in per_item.iter().enumerate() {
                let suspect: Vec<char> = items[idx].suspect.chars().collect();
                let mut extra: Vec<(f32, String)> = Vec::new();
                for (pos, top) in per_pos {
                    for &(c, v) in top.iter().filter(|(c, _)| *c != suspect[*pos]).take(8) {
                        let mut s = suspect.clone();
                        s[*pos] = c;
                        extra.push((v, s.into_iter().collect()));
                    }
                }
                if suspect.len() >= 2 {
                    // 位置ごとに、(元の字 + 字形の上位 2 つ)の組み合わせのうち、2 文字以上が変わるもの。
                    let mut combos: Vec<(f32, String)> = vec![(0.0, String::new())];
                    #[allow(clippy::needless_range_loop)]
                    for pos in 0..suspect.len() {
                        let mut alts: Vec<(f32, char)> = vec![(
                            sims.get(&(idx, pos, suspect[pos])).copied().unwrap_or(0.0),
                            suspect[pos],
                        )];
                        if let Some((_, top)) = per_pos.iter().find(|(p, _)| *p == pos) {
                            alts.extend(
                                top.iter()
                                    .filter(|(c, _)| *c != suspect[pos])
                                    .take(2)
                                    .map(|&(c, v)| (v, c)),
                            );
                        }
                        let mut next = Vec::new();
                        for (sc, st) in &combos {
                            for (s1, c) in &alts {
                                next.push((sc + s1, format!("{st}{c}")));
                            }
                        }
                        combos = next;
                    }
                    combos.retain(|(_, st)| {
                        st.chars().zip(&suspect).filter(|(a, b)| a != *b).count() >= 2
                    });
                    combos.sort_by(|a, b| b.0.total_cmp(&a.0));
                    extra.extend(combos.into_iter().take(8));
                }
                extra.sort_by(|a, b| b.0.total_cmp(&a.0));
                items[idx].extra = extra.into_iter().take(10).map(|(_, s)| s).collect();
            }
        }
    }
    if debug {
        for i in &items {
            eprintln!(
                "[fill] gap={} conf={:.0} {:?} | {:?} | {:?} extra={:?}",
                i.gap, i.conf, i.prefix, i.suspect, i.suffix, i.extra
            );
        }
    }
    // サーバーでの採点。字形の証拠があるときは、言語モデルが自由に挙げる候補(誤読と無関係な字が多い)は使わず、
    // 字形で上位の候補・形の似た字・検索結果から採掘した文字列だけを採点する。
    let results = match target.fill(&items, use_visual, settings.fill_web, !use_visual) {
        Ok(r) => r,
        Err(e) => {
            if debug {
                eprintln!("[fill] failed: {e}");
            }
            return 0;
        },
    };
    // 採点された候補のうち、まだ類似度が無い (位置, 文字) を、まとめて 1 回で求める。
    if use_visual {
        let mut need: std::collections::BTreeMap<(usize, usize), String> =
            std::collections::BTreeMap::new();
        for r in &results {
            let suspect: Vec<char> = items[r.index].suspect.chars().collect();
            for c in &r.all {
                let cc: Vec<char> = c.text.chars().collect();
                if cc.len() != suspect.len() {
                    continue;
                }
                for pos in 0..cc.len() {
                    if cc[pos] != suspect[pos]
                        && !sims.contains_key(&(r.index, pos, cc[pos]))
                        && cells
                            .get(r.index)
                            .and_then(|cs| cs.get(pos))
                            .is_some_and(|c| c.as_ref().is_some_and(|c| c.evidence >= 0.2))
                    {
                        let e = need.entry((r.index, pos)).or_default();
                        if !e.contains(cc[pos]) {
                            e.push(cc[pos]);
                        }
                    }
                }
            }
        }
        let keys: Vec<(usize, usize)> = need.keys().copied().collect();
        let maps: Vec<&[f32]> = keys
            .iter()
            .filter_map(|&(i, p)| cells[i][p].as_ref().map(|c| &c.map[..]))
            .collect();
        let queries: Vec<String> = keys.iter().map(|k| need[k].clone()).collect();
        if let Some(ranked) = matcher.rank(&maps, &queries, 1) {
            for (&(idx, pos), (_, q)) in keys.iter().zip(ranked) {
                for (c, v) in q {
                    sims.insert((idx, pos, c), v);
                }
            }
        }
    }
    // 決定: 言語モデル(+検索)の対数尤度の差に、字形の類似度の差(証拠の強さで重み付け)を足す。
    // 精度優先: 字形の証拠が強い(升目にインクが残っている)ときは、「字形が元の読みより明らかに悪い候補」は採らない。
    // 字形の証拠が無い(ほぼ消えている)ときは、言語モデルだけで決めるので、より大きい差(6 nat)を求める。
    let decided: Vec<FillResult> = results
        .into_iter()
        .map(|mut r| {
            let suspect = items[r.index].suspect.clone();
            let sc: Vec<char> = suspect.chars().collect();
            let orig_lp = r.all.iter().find(|c| c.text == suspect).map(|c| c.logp).unwrap_or(r.orig_logp);
            let mut best: Option<(f32, &Scored)> = None;
            for c in &r.all {
                if c.text == suspect {
                    continue;
                }
                let cc: Vec<char> = c.text.chars().collect();
                let (mut ev, mut dsim, mut worse) = (1.0f32, 0.0f32, false);
                if cc.len() == sc.len() {
                    let mut any = false;
                    for pos in 0..sc.len() {
                        if sc[pos] == cc[pos] {
                            continue;
                        }
                        let cell = cells.get(r.index).and_then(|cs| cs.get(pos)).and_then(|c| c.as_ref());
                        let (Some(cell), Some(&s_new), Some(&s_old)) = (cell, sims.get(&(r.index, pos, cc[pos])), sims.get(&(r.index, pos, sc[pos]))) else {
                            ev = 0.0;
                            continue;
                        };
                        any = true;
                        ev = ev.min(cell.evidence);
                        dsim += cell.evidence * (s_new - s_old);
                        if cell.evidence >= 0.5 && s_new < s_old - 0.02 {
                            worse = true;
                        }
                    }
                    if !any {
                        ev = 0.0;
                    }
                } else {
                    ev = 0.0;
                }
                let lm = c.logp - orig_lp;
                let diff = lm + VISUAL_WEIGHT * dsim;
                let need = if ev >= 0.5 { 3.0 } else { 3.0 + 3.0 * (1.0 - ev) };
                if debug {
                    eprintln!("[fill]   {:?}->{:?} lm+web={lm:+.1} ev={ev:.2} dsim={dsim:+.2} total={diff:+.1} need={need:.1}{}", suspect, c.text, if worse { " (visually worse)" } else { "" });
                }
                if !worse && diff >= need && best.is_none_or(|(b, _)| diff > b) {
                    best = Some((diff, c));
                }
            }
            match best {
                Some((_, c)) => {
                    r.changed = true;
                    r.chosen = c.text.clone();
                }
                None => r.changed = false,
            }
            r
        })
        .collect();
    if debug {
        eprintln!(
            "[fill] results: {:?}",
            decided
                .iter()
                .filter(|r| r.changed)
                .map(|r| (r.index, &r.chosen))
                .collect::<Vec<_>>()
        );
    }
    apply(pages, &spots, &decided)
}

/// 文字とそのスコアの並び(複雑な型を避ける別名)。
type CharScores = Vec<(char, f32)>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OcrWord;

    fn word(text: &str, x: i32, w: i32, conf: f32) -> OcrWord {
        OcrWord {
            text: text.into(),
            x,
            y: 0,
            w,
            h: 20,
            conf,
        }
    }

    fn line(text: &str, words: Vec<OcrWord>) -> OcrLine {
        OcrLine {
            text: text.into(),
            x: 0,
            y: 0,
            w: 400,
            h: 20,
            conf: 80.0,
            words,
        }
    }

    #[test]
    fn collects_low_confidence_words_with_context_and_applies_replacements() {
        let pages = vec![vec![line(
            "本曰は晴天です",
            vec![
                word("本", 0, 20, 95.0),
                word("曰", 20, 20, 40.0),
                word("は晴天です", 40, 100, 95.0),
            ],
        )]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].1.suspect, "曰");
        assert_eq!(spots[0].1.prefix, "本");
        assert_eq!(spots[0].1.suffix, "は晴天です");
        let mut pages = pages;
        let n = apply(
            &mut pages,
            &spots,
            &[FillResult {
                index: 0,
                chosen: "日".into(),
                changed: true,
                orig_logp: 0.0,
                all: Vec::new(),
            }],
        );
        assert_eq!(n, 1);
        assert_eq!(pages[0][0].text, "本日は晴天です");
        assert_eq!(pages[0][0].words[1].text, "日", "words are what gets drawn");
    }

    #[test]
    fn detects_a_gap_between_adjacent_cjk_words_and_inserts() {
        let pages = vec![vec![line(
            "本は晴天です",
            vec![word("本", 0, 20, 95.0), word("は晴天です", 40, 100, 95.0)],
        )]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1, "{spots:?}");
        assert!(spots[0].1.gap && spots[0].0.len == 0 && spots[0].0.start == 1);
        let mut pages = pages;
        apply(
            &mut pages,
            &spots,
            &[FillResult {
                index: 0,
                chosen: "日".into(),
                changed: true,
                orig_logp: 0.0,
                all: Vec::new(),
            }],
        );
        assert_eq!(pages[0][0].text, "本日は晴天です");
        assert_eq!(
            pages[0][0].words[0].text, "本日",
            "appended to the previous word"
        );
    }

    #[test]
    fn deletes_a_spurious_word() {
        let mut pages = vec![vec![line(
            "私は人毎朝",
            vec![
                word("私は", 0, 40, 95.0),
                word("人", 40, 20, 30.0),
                word("毎朝", 60, 40, 95.0),
            ],
        )]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1);
        apply(
            &mut pages,
            &spots,
            &[FillResult {
                index: 0,
                chosen: String::new(),
                changed: true,
                orig_logp: 0.0,
                all: Vec::new(),
            }],
        );
        assert_eq!(pages[0][0].text, "私は毎朝");
        assert_eq!(pages[0][0].words.len(), 2);
    }

    #[test]
    fn respects_the_spot_limit_and_ignores_confident_words() {
        let words: Vec<OcrWord> = (0..10).map(|i| word("ab", i * 30, 30, 30.0)).collect();
        let pages = vec![vec![line("ab ab ab ab ab ab ab ab ab ab", words)]];
        assert_eq!(collect(&pages, 80.0, 3).len(), 3);
        let sure = vec![vec![line("ab", vec![word("ab", 0, 30, 99.0)])]];
        assert!(collect(&sure, 80.0, 3).is_empty());
    }

    #[test]
    fn scan_spots_inside_a_confident_word_are_found_and_fixed() {
        // 1 つの単語(行全体)の中の「耕」だけが不自然、と走査が言った場合。
        let mut pages = vec![vec![line(
            "私は耕朝に出かけます",
            vec![word("私は耕朝に出かけます", 0, 200, 90.0)],
        )]];
        assert!(
            collect(&pages, 80.0, 10).is_empty(),
            "the OCR itself is confident"
        );
        let asked = scan_items(&pages);
        assert_eq!(asked.len(), 1);
        let results = vec![ScanResult {
            index: 0,
            spots: vec![ScanSpot { start: 2, len: 1 }],
        }];
        let spots = spots_from_scan(&pages, &asked, &results, &[], 10);
        assert_eq!(spots.len(), 1);
        assert_eq!((spots[0].1.suspect.as_str(), spots[0].0.woff), ("耕", 2));
        assert_eq!(spots[0].1.prefix, "私は");
        apply(
            &mut pages,
            &spots,
            &[FillResult {
                index: 0,
                chosen: "毎".into(),
                changed: true,
                orig_logp: 0.0,
                all: Vec::new(),
            }],
        );
        assert_eq!(pages[0][0].text, "私は毎朝に出かけます");
        assert_eq!(pages[0][0].words[0].text, "私は毎朝に出かけます");
    }

    #[test]
    fn scan_spots_overlapping_existing_ones_are_skipped_and_ranges_may_cross_words() {
        let pages = vec![vec![line(
            "ABCD",
            vec![word("AB", 0, 40, 90.0), word("CD", 40, 40, 90.0)],
        )]];
        let asked = scan_items(&pages);
        // 既存の箇所(0〜1 文字目)と重なるものは除く。
        let existing = vec![(
            Spot {
                page: 0,
                line: 0,
                start: 0,
                len: 2,
                word: 0,
                woff: 0,
            },
            FillItem {
                prefix: String::new(),
                suspect: "AB".into(),
                suffix: String::new(),
                gap: false,
                conf: 10.0,
                extra: Vec::new(),
            },
        )];
        let r2 = vec![ScanResult {
            index: 0,
            spots: vec![ScanSpot { start: 0, len: 1 }, ScanSpot { start: 2, len: 1 }],
        }];
        let got = spots_from_scan(&pages, &asked, &r2, &existing, 10);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0.start, 2);
    }

    #[test]
    fn a_replacement_across_several_words_merges_them_and_longer_text_is_allowed() {
        // 再 OCR のように 1 文字ずつの単語になっている行で、連続 3 文字(耕朝に)を 2 文字(毎朝)へ。
        let words = vec![
            word("私", 0, 20, 90.0),
            word("は", 20, 20, 90.0),
            word("耕", 40, 20, 40.0),
            word("朝", 60, 20, 40.0),
            word("に", 80, 20, 40.0),
            word("行", 100, 20, 90.0),
        ];
        let mut pages = vec![vec![line("私は耕朝に行く", words)]];
        let asked = scan_items(&pages);
        let results = vec![ScanResult {
            index: 0,
            spots: vec![ScanSpot { start: 2, len: 3 }],
        }];
        let spots = spots_from_scan(&pages, &asked, &results, &[], 10);
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].1.suspect, "耕朝に");
        apply(
            &mut pages,
            &spots,
            &[FillResult {
                index: 0,
                chosen: "毎朝".into(),
                changed: true,
                orig_logp: 0.0,
                all: Vec::new(),
            }],
        );
        assert_eq!(pages[0][0].text, "私は毎朝行く");
        let texts: Vec<&str> = pages[0][0].words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["私", "は", "毎朝", "行"],
            "the three words are merged into one"
        );
        let m = &pages[0][0].words[2];
        assert_eq!(
            (m.x, m.w),
            (40, 60),
            "the merged word covers the union of the boxes"
        );
    }
}
