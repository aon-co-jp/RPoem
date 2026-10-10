//! 組み立ての煙試験: 白紙のページに、OCR の結果(計画)を書き込んで、PDF として読み直せることを確かめる。
//! フォントは e2e 試験のキャッシュ(`%TEMP%/pdf-r2l-e2e-cache/fonts`)にあるときだけ使い、無ければ何もしない。

use lopdf::{dictionary, Document, Object, Stream};
use open_runo_pdf_ocr::compose::{compose, LinePlan, PagePlan};
use open_runo_pdf_ocr::style::LineStyle;
use open_runo_pdf_ocr::{OcrLine, OcrWord};
use std::sync::Arc;

fn blank_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"0.9 g 0 0 100 100 re f".to_vec(),
    ));
    let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Contents"=>content});
    doc.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let cat = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", cat);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap_or_default();
    out
}

fn word(text: &str, x: i32, w: i32) -> OcrWord {
    OcrWord {
        text: text.to_string(),
        x,
        y: 20,
        w,
        h: 30,
        conf: 95.0,
    }
}

#[test]
fn composes_and_reloads() {
    let font_path = std::env::temp_dir()
        .join("pdf-r2l-e2e-cache")
        .join("fonts")
        .join("NotoSans-Regular.ttf");
    let Ok(font) = std::fs::read(&font_path) else {
        return;
    };
    let font = Arc::new(font);
    let mut doc = Document::load_mem(&blank_pdf()).unwrap_or_default();
    let line = OcrLine {
        text: "Hello world".into(),
        x: 10,
        y: 20,
        w: 200,
        h: 30,
        conf: 95.0,
        words: vec![word("Hello", 10, 90), word("world", 120, 90)],
    };
    let style = LineStyle {
        serif: false,
        bold: false,
        fg: [0, 0, 0],
        bg: [255, 255, 255],
        has_ink: true,
        contrast: 1.0,
        cv_bar: 0.0,
        cv_stem: 0.0,
        stem_ratio: 0.1,
        stem_px: 3.0,
    };
    // 画像 400x400px を 200x200pt のページに貼った想定(y は下向き)。
    let plan = PagePlan {
        px_to_user: [0.5, 0.0, 0.0, -0.5, 0.0, 200.0],
        lines: vec![LinePlan {
            line,
            style,
            retype: true,
        }],
    };
    let mut provider = |_: &str| -> Result<Arc<Vec<u8>>, String> { Ok(Arc::clone(&font)) };
    let report =
        compose(&mut doc, &[plan], &["eng".to_string()], &mut provider).unwrap_or_default();
    assert_eq!(report.lines_retyped, 1);
    let mut out = Vec::new();
    assert!(doc.save_to(&mut out).is_ok());
    let again = Document::load_mem(&out).unwrap_or_default();
    assert_eq!(again.get_pages().len(), 1);
    let text = again.extract_text(&[1]).unwrap_or_default();
    // ToUnicode から文字が取り出せる(検索できる)。
    assert!(
        text.contains("Hello") || text.contains("world"),
        "extracted: {text:?}"
    );
}
