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
        match serde_json::from_str::<ErrResp>(body) {
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
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid response: {e}"))
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
            serde_json::from_str(&text).map_err(|e| format!("invalid OCR response: {e}"))?;
        if resp.pages.len() != pngs.len() {
            return Err(format!(
                "OCR returned {} pages for {} images",
                resp.pages.len(),
                pngs.len()
            ));
        }
        Ok(resp.pages.into_iter().map(|p| p.lines).collect())
    }

    /// AI 文脈補完(`POST /v1/ocr/fill`)。疑わしい箇所ごとの結果を返す。サーバーに言語モデルが無いときは `Err`。
    pub fn fill(&self, items: &[crate::complete::FillItem]) -> Result<Vec<crate::complete::FillResult>, String> {
        #[derive(Deserialize)]
        struct Resp {
            results: Vec<crate::complete::FillResult>,
        }
        let body = serde_json::json!({ "items": items }).to_string();
        let text = self.post_json("/v1/ocr/fill", &body, Duration::from_secs(900))?;
        let r: Resp = serde_json::from_str(&text).map_err(|e| format!("invalid fill response: {e}"))?;
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
