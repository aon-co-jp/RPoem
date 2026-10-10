//! AI 文脈補完: OCR の自信が低い・欠けた **1〜2文字** を、前後の長い文脈から言語モデルで穴埋めする。
//!
//! 長文を書き換えるのではない。信頼度の低い単語と、単語の間の不自然な隙間(文字が欠けた疑い)だけを
//! `(前の文脈, 疑わしい文字列, 後ろの文脈)` にして、サーバー(`aruaru-llm` の `POST /v1/ocr/fill`)へ送る。
//! サーバーは候補を言語モデルで採点し、十分に尤もらしいときだけ置き換えを返す。誤りの可能性があるため、
//! 置き換えは 1 か所・最大 2 文字・元の読みより明確に尤もらしいときに限る。
//! サーバーに言語モデルが無いとき(503)は何も変えずに続ける。

use crate::model::{OcrLine, Settings};
use serde::{Deserialize, Serialize};

/// サーバーへ送る 1 か所。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FillItem {
    pub prefix: String,
    pub suspect: String,
    pub suffix: String,
    pub gap: bool,
}

/// サーバーからの結果 1 か所。
#[derive(Debug, Clone, Deserialize)]
pub struct FillResult {
    pub index: usize,
    pub chosen: String,
    pub changed: bool,
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
    /// 行の単語の番号。置き換え・削除では疑わしい単語、欠けた文字の補充では直前の単語(その末尾に足す)。
    pub word: usize,
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

/// 疑わしい箇所を集める。`below` 未満の信頼度の短い単語と、CJK の単語の間の不自然な隙間(最大 `max_spots` か所)。
pub fn collect(pages: &[Vec<OcrLine>], below: f32, max_spots: usize) -> Vec<(Spot, FillItem)> {
    let mut out = Vec::new();
    for (pi, lines) in pages.iter().enumerate() {
        for (li, line) in lines.iter().enumerate() {
            let chars: Vec<char> = line.text.chars().collect();
            let before_line = if li > 0 { tail(&lines[li - 1].text, CONTEXT_PREV) } else { String::new() };
            let after_line: String = lines.get(li + 1).map(|l| l.text.chars().take(CONTEXT_NEXT).collect()).unwrap_or_default();
            let ctx = |start: usize, len: usize| {
                let pre: String = chars[..start].iter().collect();
                let suf: String = chars[start + len..].iter().collect();
                let join = if line.text.chars().any(is_cjk) { "" } else { " " };
                (format!("{}{}{}", if before_line.is_empty() { String::new() } else { format!("{before_line}{join}") }, pre, ""), format!("{suf}{}{}", if after_line.is_empty() { "" } else { join }, after_line))
            };
            // 1 文字あたりの幅(CJK の隙間の判定用)。
            let char_w = {
                let mut ws: Vec<f32> = line.words.iter().filter(|w| !w.text.is_empty()).map(|w| w.w as f32 / w.text.chars().count() as f32).collect();
                ws.sort_by(|a, b| a.total_cmp(b));
                ws.get(ws.len() / 2).copied().unwrap_or(0.0)
            };
            let mut cursor = 0usize; // chars 内の探索開始位置
            let mut prev_end: Option<(usize, i32, usize)> = None; // (直前の単語の終わりの位置, 右端 px, 単語の番号)
            for (wi, w) in line.words.iter().enumerate() {
                let wchars: Vec<char> = w.text.chars().collect();
                if wchars.is_empty() {
                    continue;
                }
                // 行の文字列の中で、この単語の位置を探す。
                let found = (cursor..=chars.len().saturating_sub(wchars.len())).find(|&i| i + wchars.len() <= chars.len() && chars[i..i + wchars.len()] == wchars[..]);
                let Some(start) = found else {
                    continue;
                };
                cursor = start + wchars.len();
                // 欠けた文字の疑い: 直前の単語との隙間が、1 文字ぶんの幅の 0.8 倍以上、かつ文字列の上で隣り合っている CJK。
                if let Some((end_pos, right_px, prev_wi)) = prev_end {
                    let gap = w.x - right_px;
                    if end_pos == start && char_w > 0.0 && gap as f32 >= char_w * 0.8 && start > 0 && is_cjk(chars[start - 1]) && is_cjk(chars[start]) && out.len() < max_spots {
                        let (prefix, suffix) = ctx(start, 0);
                        out.push((Spot { page: pi, line: li, start, len: 0, word: prev_wi }, FillItem { prefix, suspect: String::new(), suffix, gap: true }));
                    }
                }
                prev_end = Some((start + wchars.len(), w.x + w.w, wi));
                if w.conf >= 0.0 && w.conf < below && wchars.len() <= 8 && out.len() < max_spots && wchars.iter().any(|c| c.is_alphanumeric()) {
                    let (prefix, suffix) = ctx(start, wchars.len());
                    out.push((Spot { page: pi, line: li, start, len: wchars.len(), word: wi }, FillItem { prefix, suspect: w.text.clone(), suffix, gap: false }));
                }
            }
            if out.len() >= max_spots {
                return out;
            }
        }
    }
    out
}

/// 採用された置き換えを行の文字列へ反映する(後ろの位置から順に、位置がずれないように)。戻り値は置き換えた数。
pub fn apply(pages: &mut [Vec<OcrLine>], spots: &[(Spot, FillItem)], results: &[FillResult]) -> usize {
    let mut chosen: Vec<(&Spot, &str)> = results.iter().filter(|r| r.changed).filter_map(|r| spots.get(r.index).map(|(s, _)| (s, r.chosen.as_str()))).collect();
    chosen.sort_by(|a, b| (b.0.page, b.0.line, b.0.start).cmp(&(a.0.page, a.0.line, a.0.start)));
    let mut n = 0;
    for (spot, text) in chosen {
        let Some(line) = pages.get_mut(spot.page).and_then(|p| p.get_mut(spot.line)) else {
            continue;
        };
        let mut chars: Vec<char> = line.text.chars().collect();
        if spot.start + spot.len > chars.len() || text.chars().count() > spot.len + 3 {
            continue;
        }
        chars.splice(spot.start..spot.start + spot.len, text.chars());
        line.text = chars.into_iter().collect();
        // 描画・検索用の文字は単語から作られるため、単語にも反映する(置き換え=単語の文字、削除=単語を除く、補充=直前の単語の末尾)。
        if let Some(w) = line.words.get_mut(spot.word) {
            if spot.len == 0 {
                w.text.push_str(text);
            } else if text.is_empty() {
                line.words.remove(spot.word);
            } else {
                w.text = text.to_string();
            }
        }
        n += 1;
    }
    n
}

/// 設定に従って補完する(ページ全体の OCR 結果に対して)。戻り値は置き換えた数。サーバーが使えないときは 0(何も変えない)。
pub fn run(client: &crate::client::OcrClient, pages: &mut [Vec<OcrLine>], settings: &Settings) -> usize {
    if !settings.fill {
        return 0;
    }
    let spots = collect(pages, settings.fill_below, settings.fill_max_spots);
    if spots.is_empty() {
        return 0;
    }
    let debug = std::env::var_os("PDFOCR_DEBUG").is_some();
    let items: Vec<FillItem> = spots.iter().map(|(_, i)| i.clone()).collect();
    if debug {
        for i in &items {
            eprintln!("[fill] gap={} {:?} | {:?} | {:?}", i.gap, i.prefix, i.suspect, i.suffix);
        }
    }
    let other;
    let target = if settings.fill_server_url.trim().is_empty() {
        client
    } else {
        other = crate::client::OcrClient::new(settings.fill_server_url.trim());
        &other
    };
    match target.fill(&items) {
        Ok(results) => {
            if debug {
                eprintln!("[fill] results: {results:?}");
            }
            apply(pages, &spots, &results)
        }
        Err(e) => {
            if debug {
                eprintln!("[fill] failed: {e}");
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::OcrWord;

    fn word(text: &str, x: i32, w: i32, conf: f32) -> OcrWord {
        OcrWord { text: text.into(), x, y: 0, w, h: 20, conf }
    }

    fn line(text: &str, words: Vec<OcrWord>) -> OcrLine {
        OcrLine { text: text.into(), x: 0, y: 0, w: 400, h: 20, conf: 80.0, words }
    }

    #[test]
    fn collects_low_confidence_words_with_context_and_applies_replacements() {
        let pages = vec![vec![line("本曰は晴天です", vec![word("本", 0, 20, 95.0), word("曰", 20, 20, 40.0), word("は晴天です", 40, 100, 95.0)])]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].1.suspect, "曰");
        assert_eq!(spots[0].1.prefix, "本");
        assert_eq!(spots[0].1.suffix, "は晴天です");
        let mut pages = pages;
        let n = apply(&mut pages, &spots, &[FillResult { index: 0, chosen: "日".into(), changed: true }]);
        assert_eq!(n, 1);
        assert_eq!(pages[0][0].text, "本日は晴天です");
        assert_eq!(pages[0][0].words[1].text, "日", "words are what gets drawn");
    }

    #[test]
    fn detects_a_gap_between_adjacent_cjk_words_and_inserts() {
        // 「本」と「は晴天です」の間に 1 文字ぶんの隙間(20px)があるのに、文字列では隣り合っている。
        let pages = vec![vec![line("本は晴天です", vec![word("本", 0, 20, 95.0), word("は晴天です", 40, 100, 95.0)])]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1, "{spots:?}");
        assert!(spots[0].1.gap && spots[0].0.len == 0 && spots[0].0.start == 1);
        let mut pages = pages;
        apply(&mut pages, &spots, &[FillResult { index: 0, chosen: "日".into(), changed: true }]);
        assert_eq!(pages[0][0].text, "本日は晴天です");
        assert_eq!(pages[0][0].words[0].text, "本日", "appended to the previous word");
    }

    #[test]
    fn deletes_a_spurious_word(){
        let mut pages = vec![vec![line("私は人毎朝", vec![word("私は", 0, 40, 95.0), word("人", 40, 20, 30.0), word("毎朝", 60, 40, 95.0)])]];
        let spots = collect(&pages, 80.0, 10);
        assert_eq!(spots.len(), 1);
        apply(&mut pages, &spots, &[FillResult { index: 0, chosen: String::new(), changed: true }]);
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
}
