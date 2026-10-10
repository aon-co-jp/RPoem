//! 書体の選択: 行の文字種・推定した書体系統(ゴシック/明朝)・太字から、埋め込む Noto フォントを選ぶ。
//!
//! 日本語・簡体字・繁体字・韓国語は Noto Sans CJK / Noto Serif CJK(可変フォント、太字は wght=700 で実体化)、
//! ラテン・キリル・ギリシャ文字は Noto Sans / Noto Serif。フォントは `aruaru-llm` から必要な時に取得してキャッシュする。

/// 選んだフォント。
#[derive(Debug, Clone, PartialEq)]
pub struct FontChoice {
    /// フォントのファイル名(`aruaru-llm` / `tessdata-jpn/fonts/` と同名)。
    pub file: &'static str,
    /// 可変フォントの `wght` 軸の値(固定フォントなら `None`)。
    pub wght: Option<f32>,
    /// 太字を線の太らせで疑似的に表現するか(太字のフォントが無い明朝体など)。
    pub synthetic_bold: bool,
    /// CJK フォントか。
    pub cjk: bool,
}

impl FontChoice {
    /// 文書内で同じフォントを共有するための鍵。
    pub fn key(&self) -> String {
        format!("{}@{}", self.file, self.wght.map(|w| w as i32).unwrap_or(0))
    }
}

/// ひらがな・カタカナか。
pub fn is_kana(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x309F | 0x30A0..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F)
}

/// ハングルか。
pub fn is_hangul(c: char) -> bool {
    matches!(c as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F)
}

/// 漢字(CJK 統合漢字)か。
pub fn is_han(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF | 0x20000..=0x2A6DF)
}

/// CJK の記号・全角形か。
pub fn is_cjk_symbol(c: char) -> bool {
    matches!(c as u32, 0x3000..=0x303F | 0xFF00..=0xFF65 | 0xFFE0..=0xFFEE)
}

/// 日本語・中国語・韓国語の文字を含むか。
pub fn has_cjk(text: &str) -> bool {
    text.chars()
        .any(|c| is_kana(c) || is_hangul(c) || is_han(c) || is_cjk_symbol(c))
}

/// 行の文字列・書体系統・太字・選択言語からフォントを選ぶ。
pub fn choose_font(text: &str, serif: bool, bold: bool, languages: &[String]) -> FontChoice {
    if has_cjk(text) {
        let region = if text.chars().any(is_kana) {
            "jp"
        } else if text.chars().any(is_hangul) {
            "kr"
        } else {
            // 漢字だけの行は、選択された言語から地域を決める(既定は日本語)。
            languages
                .iter()
                .find_map(|l| match l.as_str() {
                    "jpn" | "jpn_vert" => Some("jp"),
                    "chi_sim" | "chi_sim_vert" => Some("sc"),
                    "chi_tra" | "chi_tra_vert" => Some("tc"),
                    "kor" | "kor_vert" => Some("kr"),
                    _ => None,
                })
                .unwrap_or("jp")
        };
        let file = match (serif, region) {
            (false, "jp") => "NotoSansJP-VF.ttf",
            (false, "sc") => "NotoSansSC-VF.ttf",
            (false, "tc") => "NotoSansTC-VF.ttf",
            (false, _) => "NotoSansKR-VF.ttf",
            (true, "jp") => "NotoSerifJP-VF.ttf",
            (true, "sc") => "NotoSerifSC-VF.ttf",
            (true, "tc") => "NotoSerifTC-VF.ttf",
            (true, _) => "NotoSerifKR-VF.ttf",
        };
        FontChoice {
            file,
            wght: Some(if bold { 700.0 } else { 400.0 }),
            synthetic_bold: false,
            cjk: true,
        }
    } else if serif {
        FontChoice {
            file: "NotoSerif-Regular.ttf",
            wght: None,
            synthetic_bold: bold,
            cjk: false,
        }
    } else if bold {
        FontChoice {
            file: "NotoSans-Bold.ttf",
            wght: None,
            synthetic_bold: false,
            cjk: false,
        }
    } else {
        FontChoice {
            file: "NotoSans-Regular.ttf",
            wght: None,
            synthetic_bold: false,
            cjk: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn langs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn picks_by_script_and_style() {
        let f = choose_font("こんにちは", false, false, &langs(&["jpn"]));
        assert_eq!(f.file, "NotoSansJP-VF.ttf");
        assert_eq!(f.wght, Some(400.0));
        let f = choose_font("日本語", true, true, &langs(&["jpn"]));
        assert_eq!((f.file, f.wght), ("NotoSerifJP-VF.ttf", Some(700.0)));
        let f = choose_font("漢字", false, false, &langs(&["chi_tra", "eng"]));
        assert_eq!(f.file, "NotoSansTC-VF.ttf");
        let f = choose_font("한국어", false, false, &langs(&["kor"]));
        assert_eq!(f.file, "NotoSansKR-VF.ttf");
        let f = choose_font("Hello", false, true, &langs(&["eng"]));
        assert_eq!(f.file, "NotoSans-Bold.ttf");
        let f = choose_font("Hello", true, true, &langs(&["eng"]));
        assert_eq!((f.file, f.synthetic_bold), ("NotoSerif-Regular.ttf", true));
        let f = choose_font("Привет", false, false, &langs(&["rus"]));
        assert_eq!(f.file, "NotoSans-Regular.ttf");
    }
}
