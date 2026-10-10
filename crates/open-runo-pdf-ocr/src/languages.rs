//! OCR で選べる言語の一覧(Tesseract の言語コード・日本語名・英語名)と、言語選択画面に出す日英の説明文。
//! `aruaru-llm` の `ocr_data::LANGUAGES` と同じ内容(オフラインでも言語選択画面を開けるよう、アプリ側にも持つ)。

use crate::model::MAX_LANGUAGES;

/// 言語1つ: (コード, 日本語名, 英語名)。
pub type Language = (&'static str, &'static str, &'static str);

/// 選べる言語。
pub const LANGUAGES: &[Language] = &[
    ("jpn", "日本語(横書き)", "Japanese (horizontal)"),
    ("jpn_vert", "日本語(縦書き)", "Japanese (vertical)"),
    ("eng", "英語", "English"),
    ("chi_sim", "中国語(簡体字)", "Chinese (Simplified)"),
    (
        "chi_sim_vert",
        "中国語(簡体字・縦書き)",
        "Chinese (Simplified, vertical)",
    ),
    (
        "chi_tra",
        "中国語(繁体字・台湾/香港)",
        "Chinese (Traditional, Taiwan/Hong Kong)",
    ),
    (
        "chi_tra_vert",
        "中国語(繁体字・縦書き)",
        "Chinese (Traditional, vertical)",
    ),
    ("kor", "韓国語", "Korean"),
    ("kor_vert", "韓国語(縦書き)", "Korean (vertical)"),
    ("rus", "ロシア語", "Russian"),
    ("ukr", "ウクライナ語", "Ukrainian"),
    ("fas", "ペルシャ語(イラン)", "Persian (Iran)"),
    ("ara", "アラビア語", "Arabic"),
    ("deu", "ドイツ語", "German"),
    ("fra", "フランス語", "French"),
    ("spa", "スペイン語", "Spanish"),
    ("ita", "イタリア語", "Italian"),
    ("por", "ポルトガル語", "Portuguese"),
    ("nld", "オランダ語", "Dutch"),
    ("pol", "ポーランド語", "Polish"),
    ("ces", "チェコ語", "Czech"),
    ("slk", "スロバキア語", "Slovak"),
    ("hun", "ハンガリー語", "Hungarian"),
    ("ron", "ルーマニア語", "Romanian"),
    ("bul", "ブルガリア語", "Bulgarian"),
    ("ell", "ギリシャ語", "Greek"),
    ("swe", "スウェーデン語", "Swedish"),
    ("dan", "デンマーク語", "Danish"),
    ("nor", "ノルウェー語", "Norwegian"),
    ("fin", "フィンランド語", "Finnish"),
    ("hrv", "クロアチア語", "Croatian"),
    ("slv", "スロベニア語", "Slovenian"),
    ("srp", "セルビア語", "Serbian"),
    ("lit", "リトアニア語", "Lithuanian"),
    ("lav", "ラトビア語", "Latvian"),
    ("est", "エストニア語", "Estonian"),
    ("cat", "カタルーニャ語", "Catalan"),
    ("eus", "バスク語", "Basque"),
    ("glg", "ガリシア語", "Galician"),
    ("isl", "アイスランド語", "Icelandic"),
    ("gle", "アイルランド語", "Irish"),
    ("mlt", "マルタ語", "Maltese"),
    ("sqi", "アルバニア語", "Albanian"),
    ("bos", "ボスニア語", "Bosnian"),
    ("mkd", "マケドニア語", "Macedonian"),
    ("bel", "ベラルーシ語", "Belarusian"),
    ("tur", "トルコ語", "Turkish"),
    ("heb", "ヘブライ語", "Hebrew"),
    ("hin", "ヒンディー語", "Hindi"),
];

/// 言語選択画面の説明(日本語)。
pub const LANGUAGE_HELP_JA: &str = "文字認識(OCR)に使う言語を、1〜12個まで選べます。\n\
選んだ言語のデータだけを取得して保存(キャッシュ)し、次回以降は再利用するので、選ぶ言語が少ないほど速く軽くなります。\n\
・ページに含まれる言語を全て選んでください。含まれない言語まで選ぶと、誤認識が増え、処理も遅くなります(例: 日本語と英語だけの文書なら「日本語」と「英語」の2つ)。\n\
・初めて使う言語は、数MB〜十数MBのデータを取得するため、少し時間がかかります(オンライン環境が必要です)。\n\
・縦書きの文書は「縦書き」の言語も選んでください(縦書きの行は置き換えず、検索用の見えない文字だけが追加されます)。\n\
・アラビア語・ペルシャ語・ヘブライ語・ヒンディー語などは文字の形が前後で変わるため、置き換えずに検索用の見えない文字だけを追加します。";

/// 言語選択画面の説明(英語)。
pub const LANGUAGE_HELP_EN: &str = "Choose 1 to 12 languages for text recognition (OCR).\n\
Only the data for the languages you choose is downloaded and cached for reuse, so fewer languages means faster and lighter processing.\n\
- Select every language that appears in your pages. Selecting languages that do not appear increases misrecognition and slows things down (e.g. for a document in Japanese and English only, choose just those two).\n\
- A language used for the first time downloads several MB to about 15 MB of data, which takes a moment (an online environment is required).\n\
- For vertical text, also select the \"vertical\" language (vertical lines are not replaced; only invisible text for searching is added).\n\
- For Arabic, Persian, Hebrew, Hindi, etc., where letter shapes change by position, the text is not replaced; only invisible text for searching is added.";

/// 品質の説明(日本語)。
pub const QUALITY_HELP_JA: &str =
    "高精度: 認識が正確(言語データが大きく、やや遅い)。高速: 小さく速い(認識精度はやや低い)。";
/// 品質の説明(英語)。
pub const QUALITY_HELP_EN: &str = "Best: more accurate (larger language data, slightly slower). Fast: smaller and quicker (slightly less accurate).";

/// 機能全体の注意(オンライン前提など。日本語)。
pub const FEATURE_NOTE_JA: &str = "この機能はオンライン環境が必要です。ページ画像を OCR サーバー(aruaru-llm)へ送って文字を読み取り、処理後すぐ破棄されます。OCR や書体の判定には誤りがあり得ます。チェックを外すと、元のデータのまま表示します。";
/// 機能全体の注意(英語)。
pub const FEATURE_NOTE_EN: &str = "This feature requires an online environment. Page images are sent to the OCR server (aruaru-llm) to read the text and are discarded right after processing. OCR and typeface detection can be wrong. Uncheck it to keep the original data as-is.";

/// コードから (日本語名, 英語名) を引く。
pub fn name_of(code: &str) -> Option<(&'static str, &'static str)> {
    LANGUAGES.iter().find(|l| l.0 == code).map(|l| (l.1, l.2))
}

/// 言語を1つ切り替える。選択数が上限([`MAX_LANGUAGES`])を超える追加は無視し、最後の1つは外せない。
/// 戻り値は変更があったか。
pub fn toggle(selected: &mut Vec<String>, code: &str) -> bool {
    if let Some(pos) = selected.iter().position(|c| c == code) {
        if selected.len() <= 1 {
            return false;
        }
        selected.remove(pos);
        true
    } else if selected.len() < MAX_LANGUAGES {
        selected.push(code.to_string());
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_unique_and_toggle_limits() {
        let mut codes: Vec<_> = LANGUAGES.iter().map(|l| l.0).collect();
        let n = codes.len();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), n);
        assert_eq!(n, 49);
        let mut sel = vec!["jpn".to_string()];
        assert!(!toggle(&mut sel, "jpn"), "last language cannot be removed");
        for l in LANGUAGES.iter().skip(1).take(11) {
            assert!(toggle(&mut sel, l.0));
        }
        assert_eq!(sel.len(), MAX_LANGUAGES);
        assert!(!toggle(&mut sel, "hin"), "13th language is rejected");
        assert!(toggle(&mut sel, "eng"));
        assert_eq!(sel.len(), MAX_LANGUAGES - 1);
    }
}
