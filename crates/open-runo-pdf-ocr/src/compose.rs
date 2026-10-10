//! OCR の結果から PDF のページに文字を書き込む。
//!
//! - **置き換える行**: 元の行の矩形を背景色で塗りつぶし(元の画像のかすれた文字を隠し)、同じ位置に
//!   元の書体の系統に合わせた Noto フォントの**本物の文字**を、単語ごとに位置と幅を合わせて描く。
//!   文字はフォントのサブセットを PDF に埋め込み、`ToUnicode` を付けるので、検索・コピーができる。
//! - **置き換えない行**(信頼度が低い・縦書き・フォントに無い文字を含む行): 元の画像はそのまま残し、
//!   同じ位置に**見えない文字**(描画モード 3)を重ねて、検索だけできるようにする。
//!
//! 元のページの内容は `q … Q` で隔離したうえで、その後ろに上記の内容を追加する。

use crate::fonts::{choose_font, FontChoice};
use crate::model::{OcrLine, Report};
use crate::style::LineStyle;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::Arc;
use subsetter::GlyphRemapper;

/// 1行ぶんの計画。
#[derive(Debug, Clone)]
pub struct LinePlan {
    /// OCR の行。
    pub line: OcrLine,
    /// 推定した見た目。
    pub style: LineStyle,
    /// 置き換えを試みるか(偽なら見えない文字だけ)。
    pub retype: bool,
}

/// 1ページぶんの計画。
#[derive(Debug, Clone)]
pub struct PagePlan {
    /// 画像ピクセル → ユーザー空間の変換 `[a b c d e f]`。
    pub px_to_user: [f64; 6],
    /// 行。
    pub lines: Vec<LinePlan>,
}

/// フォントのバイト列を名前から取得する関数。
pub type FontProvider<'a> = dyn FnMut(&str) -> Result<Arc<Vec<u8>>, String> + 'a;

/// 1つの字形(新旧の GID ではなく元の GID)・文字・送り幅(em)。
type Glyph = (u16, char, f64);

/// 描く単位: 開始 x・幅合わせの対象とする OCR の幅・字形。
type Run = (f64, f64, Vec<Glyph>);

struct EmbedFont {
    choice: FontChoice,
    data: Arc<Vec<u8>>,
    id: ObjectId,
    resource: String,
    remapper: GlyphRemapper,
    /// 新しい GID → 元の文字(ToUnicode 用)。
    unicode: BTreeMap<u16, char>,
    /// 新しい GID → 幅(1000 単位)。
    widths: BTreeMap<u16, i64>,
}

#[derive(Default)]
struct Invisible {
    id: Option<ObjectId>,
    codes: BTreeMap<char, u16>,
}

fn n(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn user(p: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (p[0] * x + p[2] * y + p[4], p[1] * x + p[3] * y + p[5])
}

fn rg(c: [u8; 3]) -> String {
    format!(
        "{} {} {}",
        n(c[0] as f64 / 255.0),
        n(c[1] as f64 / 255.0),
        n(c[2] as f64 / 255.0)
    )
}

/// Latin の行の「高さ → フォントサイズ」と「下端 → ベースライン」の補正を、含まれる文字の種類から決める。
/// 戻り値: (行の高さ/フォントサイズ, 下端からベースラインまでの距離/フォントサイズ)
fn latin_metrics(text: &str) -> (f64, f64) {
    let desc = text.chars().any(|c| "gjpqy,;()[]{}|/@Q".contains(c));
    let asc = text
        .chars()
        .any(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || "bdfhklt!?&%$#".contains(c));
    match (asc, desc) {
        (true, true) => (0.93, 0.20),
        (true, false) => (0.72, 0.0),
        (false, true) => (0.75, 0.20),
        (false, false) => (0.53, 0.0),
    }
}

fn hex_code(code: u16) -> String {
    format!("{code:04X}")
}

fn to_unicode_cmap(entries: &BTreeMap<u16, char>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let all: Vec<(&u16, &char)> = entries.iter().collect();
    for chunk in all.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (code, ch) in chunk {
            let mut buf = [0u16; 2];
            let units = ch.encode_utf16(&mut buf);
            let mut hex = String::new();
            for u in units.iter() {
                let _ = write!(hex, "{u:04X}");
            }
            let _ = writeln!(s, "<{}> <{}>", hex_code(**code), hex);
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

fn name(s: &str) -> Object {
    Object::Name(s.as_bytes().to_vec())
}

fn effective_resources(doc: &Document, page_id: ObjectId) -> Dictionary {
    let mut id = page_id;
    for _ in 0..32 {
        let Ok(d) = doc.get_dictionary(id) else { break };
        if let Ok(r) = d.get(b"Resources") {
            let resolved = match r {
                Object::Reference(rid) => doc.get_dictionary(*rid).ok().cloned(),
                Object::Dictionary(dd) => Some(dd.clone()),
                _ => None,
            };
            if let Some(res) = resolved {
                return res;
            }
        }
        match d.get(b"Parent") {
            Ok(Object::Reference(pid)) => id = *pid,
            _ => break,
        }
    }
    Dictionary::new()
}

fn resolve_dict(doc: &Document, o: Option<&Object>) -> Dictionary {
    match o {
        Some(Object::Reference(r)) => doc.get_dictionary(*r).ok().cloned().unwrap_or_default(),
        Some(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    }
}

struct Composer<'a, 'p> {
    doc: &'a mut Document,
    languages: &'a [String],
    provider: &'a mut FontProvider<'p>,
    fonts: Vec<EmbedFont>,
    by_key: HashMap<String, usize>,
    invisible: Invisible,
}

/// 1行を描く操作列。`(白塗り, 文字)`。
struct LineOps {
    whiteout: String,
    text: String,
    retyped: bool,
}

impl<'a, 'p> Composer<'a, 'p> {
    fn font_index(&mut self, choice: &FontChoice) -> Result<usize, String> {
        let key = choice.key();
        if let Some(&i) = self.by_key.get(&key) {
            return Ok(i);
        }
        let data = (self.provider)(choice.file)?;
        let id = self.doc.new_object_id();
        let idx = self.fonts.len();
        self.fonts.push(EmbedFont {
            choice: choice.clone(),
            data,
            id,
            resource: format!("PdfR2lF{idx}"),
            remapper: GlyphRemapper::new(),
            unicode: BTreeMap::new(),
            widths: BTreeMap::new(),
        });
        self.by_key.insert(key, idx);
        Ok(idx)
    }

    /// 置き換え。フォントに無い文字があれば `None`(見えない文字にする)。
    fn retype_line(&mut self, p: &[f64; 6], lp: &LinePlan) -> Result<Option<LineOps>, String> {
        let line = &lp.line;
        let words: Vec<&crate::model::OcrWord> = line
            .words
            .iter()
            .filter(|w| !w.text.trim().is_empty())
            .collect();
        if words.is_empty() {
            return Ok(None);
        }
        let choice = choose_font(&line.text, lp.style.serif, lp.style.bold, self.languages);
        let idx = self.font_index(&choice)?;
        let data = Arc::clone(&self.fonts[idx].data);
        let Ok(mut face) = ttf_parser::Face::parse(&data, 0) else {
            return Ok(None);
        };
        if let Some(w) = choice.wght {
            let _ = face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), w);
        }
        let upem = face.units_per_em() as f64;

        // 各単語の字形と自然な幅(em 単位)。
        let mut word_glyphs: Vec<Vec<Glyph>> = Vec::new();
        for w in &words {
            let mut gl = Vec::new();
            for c in w.text.chars() {
                if c.is_whitespace() {
                    continue;
                }
                let Some(g) = face.glyph_index(c) else {
                    return Ok(None);
                };
                if g.0 == 0 {
                    return Ok(None);
                }
                let adv = face.glyph_hor_advance(g).unwrap_or(face.units_per_em()) as f64 / upem;
                gl.push((g.0, c, adv));
            }
            if gl.is_empty() {
                return Ok(None);
            }
            word_glyphs.push(gl);
        }

        // フォントサイズ(px)とベースラインの位置。
        let line_h = line.h.max(1) as f64;
        let nat_all: f64 = word_glyphs
            .iter()
            .flatten()
            .map(|x| x.2)
            .sum::<f64>()
            .max(0.05);
        let non_ws = line
            .text
            .chars()
            .filter(|c| !c.is_whitespace())
            .count()
            .max(1);
        let cjk_chars = line
            .text
            .chars()
            .filter(|&c| {
                crate::fonts::is_kana(c)
                    || crate::fonts::is_hangul(c)
                    || crate::fonts::is_han(c)
                    || crate::fonts::is_cjk_symbol(c)
            })
            .count();
        let cjk_dominant = cjk_chars * 2 >= non_ws;
        // 日本語などが主の行: OCR の行の高さは文字より大きくなりがち、単語の矩形は送り幅と一致しないため、
        // 行全体の幅と文字数(送り幅の合計)からフォントサイズを決め、縦は行の中央に合わせる。
        // 英数字が主の行: 含まれる文字の種類(大文字・小文字・g/y など)から高さ→フォントサイズを決め、下端に合わせる。
        let (fs, base_y) = if cjk_dominant {
            let h_fs = line_h / 0.93;
            let fs = (line.w as f64 / nat_all).clamp(h_fs * 0.5, h_fs * 1.3);
            (fs, line.y as f64 + line_h / 2.0 + 0.38 * fs)
        } else {
            let (hr, bo) = latin_metrics(&line.text);
            let fs = line_h / hr;
            (fs, (line.y + line.h) as f64 - bo * fs)
        };
        // 空白の無い日本語の行は、単語の区切りが不正確なので1つの連続した文字列として描く。
        let single_run = cjk_dominant && !line.text.contains(' ');

        // 白塗り(行の矩形を少し広げて、背景色で塗る)。
        let padx = (line_h * 0.15).max(2.0);
        let pady = (line_h * 0.10).max(1.0);
        let (x0, y0) = (line.x as f64 - padx, line.y as f64 - pady);
        let (x1, y1) = (
            (line.x + line.w) as f64 + padx,
            (line.y + line.h) as f64 + pady,
        );
        let c = [
            user(p, x0, y0),
            user(p, x1, y0),
            user(p, x1, y1),
            user(p, x0, y1),
        ];
        let mut whiteout = format!("{} rg\n{} {} m\n", rg(lp.style.bg), n(c[0].0), n(c[0].1));
        for pt in &c[1..] {
            let _ = writeln!(whiteout, "{} {} l", n(pt.0), n(pt.1));
        }
        whiteout.push_str("h f\n");

        // 文字。連続した文字列(1回の Tj)か、単語ごと(位置と幅を合わせる)。
        let user_scale = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let mut text = String::new();
        let _ = writeln!(text, "{} rg", rg(lp.style.fg));
        if choice.synthetic_bold {
            let _ = writeln!(
                text,
                "{} RG {} w",
                rg(lp.style.fg),
                n(fs * 0.03 * user_scale)
            );
        }
        let resource = self.fonts[idx].resource.clone();
        // 描く単位: (開始 x、幅合わせの対象とする OCR の幅、字形)。
        let runs: Vec<Run> = if single_run {
            vec![(
                line.x as f64,
                line.w as f64,
                word_glyphs.iter().flatten().copied().collect(),
            )]
        } else {
            words
                .iter()
                .zip(word_glyphs)
                .map(|(w, g)| (w.x as f64, w.w as f64, g))
                .collect()
        };
        for (run_x, run_w, gl) in runs {
            let nat: f64 = gl.iter().map(|x| x.2).sum::<f64>().max(0.05);
            let run_has_cjk = gl
                .iter()
                .any(|(_, c, _)| crate::fonts::has_cjk(&c.to_string()));
            let sx = if cjk_dominant && !single_run {
                1.0
            } else if run_has_cjk {
                (run_w / (nat * fs)).clamp(0.7, 1.0)
            } else {
                (run_w / (nat * fs)).clamp(0.6, 1.6)
            };
            let (ox, oy) = user(p, run_x, base_y);
            let tm = [
                p[0] * fs * sx,
                p[1] * fs * sx,
                -p[2] * fs,
                -p[3] * fs,
                ox,
                oy,
            ];
            let mut hex = String::new();
            for (gid, ch, adv) in gl {
                let new = self.fonts[idx].remapper.remap(gid);
                self.fonts[idx].unicode.entry(new).or_insert(ch);
                self.fonts[idx]
                    .widths
                    .entry(new)
                    .or_insert((adv * 1000.0).round() as i64);
                hex.push_str(&hex_code(new));
            }
            let _ = writeln!(
                text,
                "BT /{resource} 1 Tf {}{} {} {} {} {} {} Tm <{hex}> Tj ET",
                if choice.synthetic_bold { "2 Tr " } else { "" },
                n(tm[0]),
                n(tm[1]),
                n(tm[2]),
                n(tm[3]),
                n(tm[4]),
                n(tm[5]),
            );
        }
        Ok(Some(LineOps {
            whiteout,
            text,
            retyped: true,
        }))
    }

    /// 見えない文字(検索用)。元の画像はそのまま。
    fn invisible_line(&mut self, p: &[f64; 6], lp: &LinePlan) -> Option<LineOps> {
        let line = &lp.line;
        let line_h = line.h.max(1) as f64;
        let fs = line_h * 0.9;
        let base_y = (line.y + line.h) as f64 - 0.15 * fs;
        let mut text = String::from("3 Tr\n");
        let mut any = false;
        for w in line.words.iter().filter(|w| !w.text.trim().is_empty()) {
            let chars: Vec<char> = w.text.chars().filter(|c| !c.is_whitespace()).collect();
            if chars.is_empty() {
                continue;
            }
            let sx = (w.w as f64 / (chars.len() as f64 * fs)).clamp(0.2, 3.0);
            let (ox, oy) = user(p, w.x as f64, base_y);
            let tm = [
                p[0] * fs * sx,
                p[1] * fs * sx,
                -p[2] * fs,
                -p[3] * fs,
                ox,
                oy,
            ];
            let mut hex = String::new();
            for ch in chars {
                let next = (self.invisible.codes.len() as u32 + 1).min(0xFFFF) as u16;
                let code = *self.invisible.codes.entry(ch).or_insert(next);
                hex.push_str(&hex_code(code));
            }
            let _ = writeln!(
                text,
                "BT /PdfR2lInv 1 Tf {} {} {} {} {} {} Tm <{hex}> Tj ET",
                n(tm[0]),
                n(tm[1]),
                n(tm[2]),
                n(tm[3]),
                n(tm[4]),
                n(tm[5])
            );
            any = true;
        }
        text.push_str("0 Tr\n");
        if any {
            Some(LineOps {
                whiteout: String::new(),
                text,
                retyped: false,
            })
        } else {
            None
        }
    }

    fn finish_fonts(&mut self) -> Result<(), String> {
        for f in &self.fonts {
            let subset = match f.choice.wght {
                Some(w) => subsetter::subset_with_variations(
                    &f.data,
                    0,
                    &[(subsetter::Tag::new(b"wght"), w)],
                    &f.remapper,
                ),
                None => subsetter::subset(&f.data, 0, &f.remapper),
            }
            .map_err(|e| {
                format!(
                    "フォントのサブセット化に失敗 / font subsetting failed ({}): {e:?}",
                    f.choice.file
                )
            })?;

            let Ok(face) = ttf_parser::Face::parse(&f.data, 0) else {
                return Err("font parse".into());
            };
            let upem = face.units_per_em() as f64;
            let k = 1000.0 / upem;
            let bbox = face.global_bounding_box();
            let base_font = format!(
                "PdfR2l+{}",
                f.choice
                    .file
                    .trim_end_matches(".ttf")
                    .replace(['-', '_'], "")
            );

            let mut file_stream = Stream::new(
                {
                    let mut d = Dictionary::new();
                    d.set("Length1", Object::Integer(subset.len() as i64));
                    d
                },
                subset,
            );
            let _ = file_stream.compress();
            let file_id = self.doc.add_object(file_stream);

            let mut desc = Dictionary::new();
            desc.set("Type", name("FontDescriptor"));
            desc.set("FontName", name(&base_font));
            desc.set("Flags", Object::Integer(4));
            desc.set(
                "FontBBox",
                Object::Array(vec![
                    Object::Integer((bbox.x_min as f64 * k) as i64),
                    Object::Integer((bbox.y_min as f64 * k) as i64),
                    Object::Integer((bbox.x_max as f64 * k) as i64),
                    Object::Integer((bbox.y_max as f64 * k) as i64),
                ]),
            );
            desc.set("ItalicAngle", Object::Integer(0));
            desc.set(
                "Ascent",
                Object::Integer((face.ascender() as f64 * k) as i64),
            );
            desc.set(
                "Descent",
                Object::Integer((face.descender() as f64 * k) as i64),
            );
            desc.set(
                "CapHeight",
                Object::Integer(
                    (face.capital_height().unwrap_or(face.ascender()) as f64 * k) as i64,
                ),
            );
            desc.set(
                "StemV",
                Object::Integer(if f.choice.wght.map(|w| w >= 600.0).unwrap_or(false) {
                    140
                } else {
                    80
                }),
            );
            desc.set("FontFile2", Object::Reference(file_id));
            let desc_id = self.doc.add_object(desc);

            let mut w_arr: Vec<Object> = Vec::new();
            for (gid, w) in &f.widths {
                w_arr.push(Object::Integer(*gid as i64));
                w_arr.push(Object::Array(vec![Object::Integer(*w)]));
            }
            let mut cid = Dictionary::new();
            cid.set("Type", name("Font"));
            cid.set("Subtype", name("CIDFontType2"));
            cid.set("BaseFont", name(&base_font));
            cid.set(
                "CIDSystemInfo",
                Object::Dictionary({
                    let mut d = Dictionary::new();
                    d.set(
                        "Registry",
                        Object::String(b"Adobe".to_vec(), StringFormat::Literal),
                    );
                    d.set(
                        "Ordering",
                        Object::String(b"Identity".to_vec(), StringFormat::Literal),
                    );
                    d.set("Supplement", Object::Integer(0));
                    d
                }),
            );
            cid.set("FontDescriptor", Object::Reference(desc_id));
            cid.set("DW", Object::Integer(1000));
            cid.set("W", Object::Array(w_arr));
            cid.set("CIDToGIDMap", name("Identity"));
            let cid_id = self.doc.add_object(cid);

            let mut tu = Stream::new(Dictionary::new(), to_unicode_cmap(&f.unicode).into_bytes());
            let _ = tu.compress();
            let tu_id = self.doc.add_object(tu);

            let mut t0 = Dictionary::new();
            t0.set("Type", name("Font"));
            t0.set("Subtype", name("Type0"));
            t0.set("BaseFont", name(&base_font));
            t0.set("Encoding", name("Identity-H"));
            t0.set(
                "DescendantFonts",
                Object::Array(vec![Object::Reference(cid_id)]),
            );
            t0.set("ToUnicode", Object::Reference(tu_id));
            self.doc.objects.insert(f.id, Object::Dictionary(t0));
        }
        Ok(())
    }

    fn finish_invisible(&mut self) -> Option<ObjectId> {
        if self.invisible.codes.is_empty() {
            return None;
        }
        let entries: BTreeMap<u16, char> = self
            .invisible
            .codes
            .iter()
            .map(|(c, code)| (*code, *c))
            .collect();
        let mut desc = Dictionary::new();
        desc.set("Type", name("FontDescriptor"));
        desc.set("FontName", name("PdfR2lInvisible"));
        desc.set("Flags", Object::Integer(4));
        desc.set(
            "FontBBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(-200),
                Object::Integer(1000),
                Object::Integer(900),
            ]),
        );
        desc.set("ItalicAngle", Object::Integer(0));
        desc.set("Ascent", Object::Integer(900));
        desc.set("Descent", Object::Integer(-200));
        desc.set("CapHeight", Object::Integer(700));
        desc.set("StemV", Object::Integer(80));
        let desc_id = self.doc.add_object(desc);
        let mut cid = Dictionary::new();
        cid.set("Type", name("Font"));
        cid.set("Subtype", name("CIDFontType2"));
        cid.set("BaseFont", name("PdfR2lInvisible"));
        cid.set(
            "CIDSystemInfo",
            Object::Dictionary({
                let mut d = Dictionary::new();
                d.set(
                    "Registry",
                    Object::String(b"Adobe".to_vec(), StringFormat::Literal),
                );
                d.set(
                    "Ordering",
                    Object::String(b"Identity".to_vec(), StringFormat::Literal),
                );
                d.set("Supplement", Object::Integer(0));
                d
            }),
        );
        cid.set("FontDescriptor", Object::Reference(desc_id));
        cid.set("DW", Object::Integer(1000));
        cid.set("CIDToGIDMap", name("Identity"));
        let cid_id = self.doc.add_object(cid);
        let mut tu = Stream::new(Dictionary::new(), to_unicode_cmap(&entries).into_bytes());
        let _ = tu.compress();
        let tu_id = self.doc.add_object(tu);
        let mut t0 = Dictionary::new();
        t0.set("Type", name("Font"));
        t0.set("Subtype", name("Type0"));
        t0.set("BaseFont", name("PdfR2lInvisible"));
        t0.set("Encoding", name("Identity-H"));
        t0.set(
            "DescendantFonts",
            Object::Array(vec![Object::Reference(cid_id)]),
        );
        t0.set("ToUnicode", Object::Reference(tu_id));
        let id = self.doc.add_object(t0);
        self.invisible.id = Some(id);
        Some(id)
    }
}

/// 各ページに計画どおりの内容を書き込む。
pub fn compose(
    doc: &mut Document,
    plans: &[PagePlan],
    languages: &[String],
    provider: &mut FontProvider,
) -> Result<Report, String> {
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    let mut report = Report {
        pages: page_ids.len(),
        ..Report::default()
    };
    let mut c = Composer {
        doc,
        languages,
        provider,
        fonts: Vec::new(),
        by_key: HashMap::new(),
        invisible: Invisible::default(),
    };

    // ページごとの操作列(白塗り → 文字)。フォントの実体は最後に作るので、リソース登録も最後にまとめる。
    let mut page_ops: Vec<(ObjectId, String, bool)> = Vec::new();
    for (i, plan) in plans.iter().enumerate() {
        let Some(&page_id) = page_ids.get(i) else {
            break;
        };
        let mut whiteouts = String::new();
        let mut texts = String::new();
        let mut uses_invisible = false;
        for lp in &plan.lines {
            report.lines_found += 1;
            let ops = if lp.retype && lp.style.has_ink {
                c.retype_line(&plan.px_to_user, lp)?
            } else {
                None
            };
            let ops = match ops {
                Some(o) => Some(o),
                None => c.invisible_line(&plan.px_to_user, lp),
            };
            if let Some(o) = ops {
                if o.retyped {
                    report.lines_retyped += 1;
                } else {
                    report.lines_text_only += 1;
                    uses_invisible = true;
                }
                whiteouts.push_str(&o.whiteout);
                texts.push_str(&o.text);
            }
        }
        if !whiteouts.is_empty() || !texts.is_empty() {
            page_ops.push((page_id, format!("q\n{whiteouts}{texts}Q\n"), uses_invisible));
        }
    }

    c.finish_fonts()?;
    let inv_id = c.finish_invisible();

    let font_refs: Vec<(String, ObjectId)> =
        c.fonts.iter().map(|f| (f.resource.clone(), f.id)).collect();
    for (page_id, ops, uses_invisible) in page_ops {
        // 元の内容を q … Q で隔離し、その後ろに追加する。
        let originals = c.doc.get_page_contents(page_id);
        let q_open = c
            .doc
            .add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
        let q_close = c
            .doc
            .add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));
        let mut ours = Stream::new(Dictionary::new(), ops.into_bytes());
        let _ = ours.compress();
        let ours_id = c.doc.add_object(ours);
        let mut contents = vec![Object::Reference(q_open)];
        contents.extend(originals.into_iter().map(Object::Reference));
        contents.push(Object::Reference(q_close));
        contents.push(Object::Reference(ours_id));

        // 継承されたリソースも含めて複製し、フォントを足して、ページ自身のリソースにする。
        let mut res = effective_resources(c.doc, page_id);
        let mut fonts = resolve_dict(c.doc, res.get(b"Font").ok());
        for (rname, id) in &font_refs {
            fonts.set(rname.as_bytes(), Object::Reference(*id));
        }
        if uses_invisible {
            if let Some(id) = inv_id {
                fonts.set("PdfR2lInv", Object::Reference(id));
            }
        }
        res.set("Font", Object::Dictionary(fonts));
        let page = c
            .doc
            .get_dictionary_mut(page_id)
            .map_err(|e| e.to_string())?;
        page.set("Contents", Object::Array(contents));
        page.set("Resources", Object::Dictionary(res));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format_is_compact() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(0.5), "0.5");
        assert_eq!(n(-0.00001), "0");
        assert_eq!(n(12.34567), "12.3457");
    }

    #[test]
    fn cmap_has_entries_and_surrogates() {
        let mut m = BTreeMap::new();
        m.insert(1u16, 'A');
        m.insert(2u16, '\u{20BB7}');
        let s = to_unicode_cmap(&m);
        assert!(s.contains("<0001> <0041>"));
        assert!(s.contains("<0002> <D842DFB7>"));
    }

    #[test]
    fn latin_metrics_depend_on_letters() {
        assert_eq!(latin_metrics("Hello gap").0, 0.93);
        assert_eq!(latin_metrics("HELLO").1, 0.0);
        assert_eq!(latin_metrics("ace").0, 0.53);
    }
}
