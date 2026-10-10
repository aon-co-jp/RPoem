#![allow(clippy::expect_used, clippy::unwrap_used)]
//! 試験用: JPEG 1枚を、A4 の1ページの PDF(スキャン文書のように画像だけ)にする。
//! 使い方: `cargo run -p open-runo-pdf-ocr --example make_sample_pdf -- in.jpg out.pdf [ページ数]`

use lopdf::{dictionary, Document, Object, Stream};

fn jpeg_size(data: &[u8]) -> Option<(i64, i64)> {
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        let len = ((data[i + 2] as usize) << 8) | data[i + 3] as usize;
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let h = ((data[i + 5] as i64) << 8) | data[i + 6] as i64;
            let w = ((data[i + 7] as i64) << 8) | data[i + 8] as i64;
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let jpg = std::fs::read(&args[1]).expect("read jpg");
    let pages: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let rotate: i64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let (w, h) = jpeg_size(&jpg).expect("jpeg size");
    // 回転 90: ページを横長にして画像を反時計回りに置く(表示すると正立する)。
    let (mb_w, mb_h, cm) = if rotate == 90 {
        (841.89f32, 595.28f32, "0 595.28 -841.89 0 841.89 0")
    } else {
        (595.28f32, 841.89f32, "595.28 0 0 841.89 0 0")
    };
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..pages {
        let img = doc.add_object(Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>w,"Height"=>h,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},
            jpg.clone(),
        ));
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("q {cm} cm /Im0 Do Q").into_bytes(),
        ));
        let page = doc.add_object(dictionary! {
            "Type"=>"Page","Parent"=>pages_id,
            "MediaBox"=>vec![0.into(),0.into(),Object::Real(mb_w),Object::Real(mb_h)],
            "Contents"=>content,"Rotate"=>rotate,
            "Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>img}},
        });
        kids.push(page.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>pages as i64}),
    );
    let cat = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
    doc.trailer.set("Root", cat);
    doc.save(&args[2]).expect("save");
}
