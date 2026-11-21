//! `aruaru-llm` の OCR API(Connect: `POST /aruaru_llm.v1.Llm/Ocr` ほか。フォントも `OcrFont`)の
//! HTTP クライアント。ネットワークに届かないときは、オンライン環境が必要である旨を日英で返す。

use crate::model::{OcrLine, Quality};
use base64::Engine;
use serde::Deserialize;
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

/// 字形の照合の結果 1 升ぶん(サーバーの `OcrGlyphRank`)。
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

#[derive(serde::Deserialize)]
struct FontResp {
    sha256: String,
    data_base64: String,
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

    fn post_json(&self, path: &str, body: &str, timeout: Duration) -> Result<String, String> {
        let url = format!("{}{path}", self.base);
        let mut resp = Self::agent(timeout)
            .post(&url)
            .header("content-type", "application/json")
            // aruaru-llm の受付制御がサイト(呼び出し元)ごとに公平に順番を回すための識別。
            .header("x-tenant", &tenant_name())
            .send(body.as_bytes())
            .map_err(|e| offline_message(&self.base, &e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            // フォント(base64で約13MB)を返す OcrFont があるため、ureq 3 の既定(10MB)より上げる。
            .with_config()
            .limit(256 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(Self::read_error(status, &text));
        }
        Ok(text)
    }

    /// 選べる言語の一覧(日英名・取得済みか・説明)。
    pub fn languages(&self) -> Result<LanguageList, String> {
        let bytes = self
            .post_json(
                "/aruaru_llm.v1.Llm/OcrLanguages",
                "{}",
                Duration::from_secs(30),
            )?
            .into_bytes();
        sonic_rs::from_slice(&bytes).map_err(|e| format!("invalid response: {e}"))
    }

    /// 選んだ言語のデータを先に取得してキャッシュさせる。
    pub fn prepare(&self, languages: &[String], quality: Quality) -> Result<(), String> {
        let body =
            serde_json::json!({ "languages": languages, "quality": quality.as_str() }).to_string();
        self.post_json(
            "/aruaru_llm.v1.Llm/OcrPrepare",
            &body,
            Duration::from_secs(1800),
        )
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
        let text = self.post_json("/aruaru_llm.v1.Llm/Ocr", &body, Duration::from_secs(1800))?;
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

    /// AI 文脈補完(`Ocr Fill`)。疑わしい箇所ごとの結果を返す。`return_all` なら採点した全候補も返す(字形の照合と合わせて選び直す用)。
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
        let text = self.post_json(
            "/aruaru_llm.v1.Llm/OcrFill",
            &body,
            Duration::from_secs(1800),
        )?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid fill response: {e}"))?;
        Ok(r.results)
    }

    /// 疑わしい箇所ごとに、入り得る 1 文字の候補を広く挙げさせる(`OcrPropose`。字形で絞り込む前提)。
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
        let text = self.post_json(
            "/aruaru_llm.v1.Llm/OcrPropose",
            &body,
            Duration::from_secs(900),
        )?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid propose response: {e}"))?;
        Ok(r.results)
    }

    /// 字形の照合をサーバーに任せる(`OcrGlyphRank`、`open-cuda` の `sgemm`)。升目(`N`×`N` の濃淡)ごとに、
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
        let text = self.post_json(
            "/aruaru_llm.v1.Llm/OcrGlyphRank",
            &body,
            Duration::from_secs(300),
        )?;
        let r: Resp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid glyph-rank response: {e}"))?;
        Ok(r.results)
    }

    /// 頻出順の漢字・かな(`OcrCharset`)。字形で候補を探すときの母集団。モデルが無いときは `Err`。
    pub fn charset(&self, n: usize) -> Result<Vec<char>, String> {
        #[derive(Deserialize)]
        struct Resp {
            chars: String,
        }
        let bytes = self
            .post_json(
                "/aruaru_llm.v1.Llm/OcrCharset",
                &format!("{{\"n\":{n}}}"),
                Duration::from_secs(300),
            )?
            .into_bytes();
        let r: Resp =
            sonic_rs::from_slice(&bytes).map_err(|e| format!("invalid charset response: {e}"))?;
        Ok(r.chars.chars().collect())
    }

    /// 言語モデルによる走査(`OcrScan`): 各行の中の、文脈にとって不自然な箇所。サーバーに言語モデルが無いときは `Err`。
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
        let text = self.post_json(
            "/aruaru_llm.v1.Llm/OcrScan",
            &body,
            Duration::from_secs(900),
        )?;
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
        let body = serde_json::json!({ "name": name }).to_string();
        let text = self.post_json(
            "/aruaru_llm.v1.Llm/OcrFont",
            &body,
            Duration::from_secs(1800),
        )?;
        let resp: FontResp =
            sonic_rs::from_str(&text).map_err(|e| format!("invalid font response: {e}"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(resp.data_base64.as_bytes())
            .map_err(|e| format!("invalid font data: {e}"))?;
        {
            use sha2::Digest as _;
            let got: String = sha2::Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if !got.eq_ignore_ascii_case(&resp.sha256) {
                return Err(format!("font {name}: sha256 mismatch"));
            }
        }
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

/// `x-tenant` に入れる呼び出し元の名前。環境変数 `ARUARU_TENANT` を優先し、無ければ実行ファイル名。
/// aruaru-llm 側が英数字と `.-:_` だけを使うため、それ以外はここで落とす。
fn tenant_name() -> String {
    let raw = std::env::var("ARUARU_TENANT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_default()
        });
    let s: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
        .take(64)
        .collect();
    if s.is_empty() {
        "anonymous".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tenant_tests {
    use super::{tenant_name, OcrClient};

    #[test]
    fn tenant_name_is_never_empty_and_is_header_safe() {
        let t = tenant_name();
        assert!(!t.is_empty() && t.len() <= 64);
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_')));
    }

    #[test]
    fn languages_call_sends_x_tenant_header() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let h = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = sock.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            let body = "{\"languages\":[]}";
            let _ = write!(
                sock,
                "HTTP/1.1 200 OK
content-type: application/json
content-length: {}
connection: close

{body}",
                body.len()
            );
            req
        });
        let c = OcrClient::new(&format!("http://{addr}"));
        let _ = c.languages();
        let req = h.join().unwrap();
        assert!(req.contains("x-tenant: "), "request was: {req}");
    }
}
