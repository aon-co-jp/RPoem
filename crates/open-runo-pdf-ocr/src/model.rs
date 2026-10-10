//! 設定・OCR 結果・進捗などの共通データ型。

use serde::{Deserialize, Serialize};

/// 1回の OCR で選べる言語数の上限(`aruaru-llm` の `MAX_LANGS` と同じ)。
pub const MAX_LANGUAGES: usize = 12;

/// OCR の品質。`Fast` は小さく速い言語データ、`Best` は高精度(大きい)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// 高精度(既定)。
    Best,
    /// 高速。
    Fast,
}

impl Quality {
    /// サーバーへ送る文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Quality::Best => "best",
            Quality::Fast => "fast",
        }
    }
}

/// 「テキストフォント文字化」の設定。アプリの設定画面で編集し、JSON で保存する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 文字化機能を使うか(既定: オン)。オフなら元のデータのまま。
    pub enabled: bool,
    /// OCR を行う `aruaru-llm` の URL(既定: 同じ PC で動く `http://127.0.0.1:4600`)。
    pub server_url: String,
    /// OCR する言語(1〜12個、Tesseract の言語コード)。
    pub languages: Vec<String>,
    /// OCR の品質。
    pub quality: Quality,
    /// 見える文字を、元の書体に合わせた文字に置き換えるか(既定: する)。偽なら、元の画像はそのまま残し、
    /// 検索・コピー用の見えない文字だけを追加する(誤認識で読める文字を壊したくないとき向け)。
    pub replace_text: bool,
    /// この信頼度(0〜100)未満の行は、元の画像のまま残す(検索用の見えない文字だけ追加する)。
    pub min_confidence: f32,
    /// ページ画像の長辺ピクセル数の上限(OCR 精度と速度のバランス)。
    pub render_long_side_px: u32,
    /// 1回の OCR 要求にまとめるページ数(1〜16)。大きいほど Tesseract の起動回数が減る。
    pub batch_pages: usize,
    /// AI 文脈補完(かすれた・読めない 1〜2 文字を前後の文脈から穴埋め、既定: オフ)。サーバーに言語モデルが必要。
    pub fill: bool,
    /// 補完を行うサーバーの URL(空なら OCR と同じ `server_url`)。OCR は VPS、言語モデルは手元の PC、という使い分け用。
    pub fill_server_url: String,
    /// この信頼度(0〜100)未満の短い単語を補完の対象にする。
    pub fill_below: f32,
    /// 1 回の変換で補完を試す箇所の上限(言語モデルは CPU では 1 か所あたり数秒かかる)。
    pub fill_max_spots: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            server_url: "http://127.0.0.1:4600".to_string(),
            languages: vec!["jpn".to_string(), "eng".to_string()],
            quality: Quality::Best,
            replace_text: true,
            min_confidence: 70.0,
            render_long_side_px: 3000,
            batch_pages: 8,
            fill: false,
            fill_server_url: String::new(),
            fill_below: 80.0,
            fill_max_spots: 60,
        }
    }
}

impl Settings {
    /// 設定の検証(言語数 1〜12、URL、バッチ数)。
    pub fn validate(&self) -> Result<(), String> {
        if self.languages.is_empty() || self.languages.len() > MAX_LANGUAGES {
            return Err(format!("言語は1〜{MAX_LANGUAGES}個選んでください / Select 1 to {MAX_LANGUAGES} languages (selected: {})", self.languages.len()));
        }
        if !(self.server_url.starts_with("http://") || self.server_url.starts_with("https://")) {
            return Err("接続先URLは http:// または https:// で始めてください / The server URL must start with http:// or https://".to_string());
        }
        if self.batch_pages == 0 || self.batch_pages > 16 {
            return Err("batch_pages must be 1..=16".to_string());
        }
        Ok(())
    }
}

/// OCR で認識した1単語(画像ピクセル座標、左上原点)。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OcrWord {
    /// 認識した文字列。
    pub text: String,
    /// 左端(px)。
    pub x: i32,
    /// 上端(px)。
    pub y: i32,
    /// 幅(px)。
    pub w: i32,
    /// 高さ(px)。
    pub h: i32,
    /// 信頼度(0〜100)。
    pub conf: f32,
}

/// OCR で認識した1行。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OcrLine {
    /// 行の文字列。
    pub text: String,
    /// 左端(px)。
    pub x: i32,
    /// 上端(px)。
    pub y: i32,
    /// 幅(px)。
    pub w: i32,
    /// 高さ(px)。
    pub h: i32,
    /// 行の平均信頼度(0〜100)。
    pub conf: f32,
    /// 行内の単語。
    pub words: Vec<OcrWord>,
}

/// 処理の段階(進捗表示用)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// 言語データの準備(初回は取得に時間がかかる)。
    Prepare,
    /// ページを画像にして OCR している。
    Ocr,
    /// 書体を取得して PDF を組み立てている。
    Compose,
}

/// 進捗。
#[derive(Debug, Clone, Copy)]
pub struct Progress {
    /// 段階。
    pub stage: Stage,
    /// 完了したページ数。
    pub done: usize,
    /// 全ページ数。
    pub total: usize,
}

/// 処理結果の概要。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// ページ数。
    pub pages: usize,
    /// OCR が見つけた行の数。
    pub lines_found: usize,
    /// 元の書体に合わせた文字へ置き換えた行の数。
    pub lines_retyped: usize,
    /// 置き換えず、元の画像のまま検索用の見えない文字だけ追加した行の数。
    pub lines_text_only: usize,
    /// AI 文脈補完で、読みを直した(または欠けた文字を補った)箇所の数。
    pub chars_filled: usize,
}
