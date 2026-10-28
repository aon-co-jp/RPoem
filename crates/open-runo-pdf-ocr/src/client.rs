//! `aruaru-llm` の OCR API(`/v1/ocr`・`/v1/ocr/languages`・`/v1/ocr/prepare`・`/v1/ocr/fonts/:name`)の
//! HTTP クライアント。ネットワークに届かないときは、オンライン環境が必要である旨を日英で返す。

use crate::model::{OcrLine, Quality};
use base64::Engine;
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// `aruaru-llm` への接続。
#[derive(Debug, Clone)]
pub struct OcrClient {
    base: String,
}

/// 選べる言語1つ(サーバーの一覧)。
#[derive(Debug, Clone, Deserialize)]
pub struct LanguageEntry {
    /// Tesseract の言語コード。
    pub code: String,
    /// 日本語名。
    pub name_ja: String,
    /// 英語名。
    pub name_en: String,
    /// 高精度版が取得済みか。
    pub cached_best: bool,
    /// 高速版が取得済みか。
    pub cached_fast: bool,
}

/// 字形の照合の結果 1 升ぶん(サーバーの `POST /v1/ocr/glyph-rank`)。
#[derive(Debug, Clone, Deserialize)]
pub struct GlyphRank {
    /// 類似度の高い上位の文字(文字, 類似度)。
    pub top: Vec<(String, f32)>,
    /// 問い合わせた文字の類似度。
    pub query: Vec<(String, f32)>,
}

/// 言語一覧と、言語選択画面に出す日英の説明。
#[derive(Debug, Clone, Deserialize)]
pub struct LanguageList {
    /// 一度に選べる最大数。
    pub max_languages: usize,
    /// サーバーで Tesseract が使えるか。
    pub tesseract_available: bool,
    /// 言語の一覧。
    pub languages: Vec<LanguageEntry>,
    /// 日本語の説明。
    pub explanation_ja: String,
    /// 英語の説明。
    pub explanation_en: String,
}

#[derive(Deserialize)]
struct OcrPageResp {
    lines: Vec<OcrLine>,
}

#[derive(Deserialize)]
struct OcrResp {
    pages: Vec<OcrPageResp>,
}

#[derive(Deserialize)]
struct ErrResp {
    error: String,
}

fn offline_message(base: &str, detail: &str) -> String {
    format!(
        "オンライン環境が必要です。OCRサーバー({base})に接続できません: {detail}\n\
         An online environment is required. Cannot reach the OCR server ({base}): {detail}"
    )
}

impl OcrClient {
    /// 接続先 URL(末尾の `/` は除く)を指定して作る。
    pub fn new(base_url: &str) -> Self {
        OcrClient {
            base: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn agent(timeout: Duration) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn read_error(status: u16, body: &str) -> String {
        match sonic_rs::from_str::<ErrResp>(body) {
            Ok(e) => format!(
                "サーバーエラー({status}) / Server error ({status}): {}",
                e.error
            ),
            Err(_) => format!("サーバーエラー({status}) / Server error ({status})"),
        }
    }

    fn get_bytes(&self, path: &str, timeout: Duration) -> Result<Vec<u8>, String> {
        let url = format!("{}{path}", self.base);
        let mut resp = Self::agent(timeout)
            .get(&url)
            .call()
            .map_err(|e| offline_message(&self.base, &e.to_string()))?;
        let status = resp.status().as_u16();
        let mut buf = Vec::new();
        resp.body_mut()
            .as_reader()
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(Self::read_error(status, &String::from_utf8_lossy(&buf)));
        }
        Ok(buf)
    }

    fn post_json(&self, path: &str, body: &str, timeout: Duration) -> Result<String, String> {
        let url = format!("{}{path}", self.base);
        let mut resp = Self::agent(timeout)
            .post(&url)
            .header("content-type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| offline_message(&self.base, &e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(Self::read_error(status, &text));
        }
        Ok(text)
    }

    /// 選べる言語の一覧(日英名・取得済みか・説明)。
    pub fn languages(&self) -> Result<LanguageList, String> {
        let bytes = self.get_bytes("/v1/ocr/languages", Duration::from_secs(30))?;
        sonic_rs::from_slice(&bytes).map_err(|e| format!("invalid response: {e}"))
    }

    /// 選んだ言語のデータを先に取得してキャッシュさせる。
    pub fn prepare(&self, languages: &[String], quality: Quality) -> Result<(), String> {
        let body =
            serde_json::json!({ "languages": languages, "quality": quality.as_str() }).to_string();
        self.post_json("/v1/ocr/prepare", &body, Duration::from_secs(1800))
            .map(|_| ())
    }

    /// 複数ページの PNG をまとめて OCR する(1回の要求 = サーバーで tesseract 1回起動)。
    /// 戻り値は画像と同じ順のページごとの行。
    pub fn ocr(
        &self,
        pngs: &[Vec<u8>],
        languages: &[String],
        quality: Quality,
    ) -> Result<Vec<Vec<OcrLine>>, String> {
        let engine = base64::engine::general_purpose::STANDARD;
        let images: Vec<String> = pngs.iter().map(|p| engine.encode(p)).collect();
        let body = serde_json::json!({ "images_base64": images, "languages": languages, "quality": quality.as_str() }).to_string();
        let text = self.post_json("/v1/ocr", &body, Duration::from_secs(1800))?;
        let resp: OcrResp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid OCR response: {e}"))?;
        if resp.pages.len() != pngs.len() {
            return Err(format!(
                "OCR returned {} pages for {} images",
                resp.pages.len(),
                pngs.len()
            ));
        }
        Ok(resp.pages.into_iter().map(|p| p.lines).collect())
    }

    /// AI 文脈補完(`POST /v1/ocr/fill`)。疑わしい箇所ごとの結果を返す。`return_all` なら採点した全候補も返す(字形の照合と合わせて選び直す用)。
    /// サーバーに言語モデルが無いときは `Err`。
    pub fn fill(
        &self,
        items: &[crate::complete::FillItem],
        return_all: bool,
        use_web: bool,
        lm_free: bool,
    ) -> Result<Vec<crate::complete::FillResult>, String> {
        #[derive(Deserialize)]
        struct Resp {
            results: Vec<crate::complete::FillResult>,
        }
        let body = serde_json::json!({ "items": items, "return_all": return_all, "use_web": use_web, "lm_candidates": lm_free }).to_string();
        let text = self.post_json("/v1/ocr/fill", &body, Duration::from_secs(1800))?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid fill response: {e}"))?;
        Ok(r.results)
    }

    /// 疑わしい箇所ごとに、入り得る 1 文字の候補を広く挙げさせる(`POST /v1/ocr/propose`。字形で絞り込む前提)。
    pub fn propose(
        &self,
        items: &[crate::complete::FillItem],
        k: usize,
    ) -> Result<Vec<crate::complete::ProposeResult>, String> {
        #[derive(Deserialize)]
        struct Resp {
            results: Vec<crate::complete::ProposeResult>,
        }
        let body = serde_json::json!({ "items": items, "k": k }).to_string();
        let text = self.post_json("/v1/ocr/propose", &body, Duration::from_secs(900))?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid propose response: {e}"))?;
        Ok(r.results)
    }

    /// 字形の照合をサーバーに任せる(`POST /v1/ocr/glyph-rank`、`open-cuda` の `sgemm`)。升目(`N`×`N` の濃淡)ごとに、
    /// 類似度の高い上位の文字と、`query[i]` の各文字の類似度を返す。サーバーが対応していない・失敗のときは `Err`(アプリ内の計算に切り替える)。
    pub fn glyph_rank(
        &self,
        cells: &[Vec<f32>],
        query: &[String],
        extra_chars: &str,
        top: usize,
    ) -> Result<Vec<GlyphRank>, String> {
        #[derive(Deserialize)]
        struct Resp {
            results: Vec<GlyphRank>,
        }
        let cells_json: Vec<serde_json::Value> = cells
            .iter()
            .map(|m| serde_json::json!({ "map": m }))
            .collect();
        let body = serde_json::json!({ "cells": cells_json, "query": query, "extra_chars": extra_chars, "top": top }).to_string();
        let text = self.post_json("/v1/ocr/glyph-rank", &body, Duration::from_secs(300))?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid glyph-rank response: {e}"))?;
        Ok(r.results)
    }

    /// 頻出順の漢字・かな(`GET /v1/ocr/charset`)。字形で候補を探すときの母集団。モデルが無いときは `Err`。
    pub fn charset(&self, n: usize) -> Result<Vec<char>, String> {
        #[derive(Deserialize)]
        struct Resp {
            chars: String,
        }
        let bytes = self.get_bytes(&format!("/v1/ocr/charset?n={n}"), Duration::from_secs(300))?;
        let r: Resp =
            sonic_rs::from_slice(&bytes).map_err(|e| format!("invalid charset response: {e}"))?;
        Ok(r.chars.chars().collect())
    }

    /// 言語モデルによる走査(`POST /v1/ocr/scan`): 各行の中の、文脈にとって不自然な箇所。サーバーに言語モデルが無いときは `Err`。
    pub fn scan(
        &self,
        items: &[crate::complete::ScanItem],
    ) -> Result<Vec<crate::complete::ScanResult>, String> {
        #[derive(Deserialize)]
        struct Resp {
            results: Vec<crate::complete::ScanResult>,
        }
        let body =
            serde_json::json!({ "items": items, "threshold": -8.0, "max_spots": 4 }).to_string();
        let text = self.post_json("/v1/ocr/scan", &body, Duration::from_secs(900))?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid scan response: {e}"))?;
        Ok(r.results)
    }

    /// 書体フォントを取得する。`cache_dir/fonts/<name>` にあればそれを使い、無ければサーバーから取得して保存する。
    pub fn font(&self, name: &str, cache_dir: &Path) -> Result<Vec<u8>, String> {
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err("invalid font name".to_string());
        }
        let path = cache_dir.join("fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.len() > 100_000 {
                return Ok(bytes);
            }
        }
        let bytes = self.get_bytes(&format!("/v1/ocr/fonts/{name}"), Duration::from_secs(1800))?;
        if bytes.len() < 100_000 {
            return Err(format!("font {name} is unexpectedly small"));
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("part");
        if std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        Ok(bytes)
    }
}
