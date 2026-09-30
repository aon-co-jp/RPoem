//! 旧Kotlin版の音声処理(`VoiceDsp.render`)の出力(`tests/golden/*.f32`、float32リトルエンディアン)と、
//! Rust版の出力を数値で照合する。「テストが通る」だけでなく「同じ入力で同じ音が出る」ことの保証。
//! 旧方式(`Mode::Legacy`)との照合。既定の新方式(音程と声の太さを独立に制御)は`tests/formant.rs`で検証する。
//! フィクスチャはKotlin版で生成したもの(Kotlin版は移行後に削除されるが、この基準は回帰防止として残す)。

use open_runo_voice::dsp::{recipe, render_with_recipe, Mode, Recipe, SourceGender};
use open_runo_voice::wav::Pcm;
use open_runo_voice::VoiceStyle;
use std::f64::consts::PI;

const SR: u32 = 22050;

fn voiced(f0: f64, seconds: f64) -> Vec<f32> {
    (0..(SR as f64 * seconds) as usize)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let env = 0.6 + 0.4 * (2.0 * PI * 3.0 * t).sin();
            let v: f64 = (1..=5)
                .map(|h| (2.0 * PI * f0 * h as f64 * t).sin() / h as f64)
                .sum();
            (env * v * 0.3) as f32
        })
        .collect()
}

fn golden(name: &str) -> Vec<f32> {
    let path = format!("{}/tests/golden/{name}.f32", env!("CARGO_MANIFEST_DIR"));
    let b = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// (最大絶対誤差, 誤差のRMS/正解のRMS)
fn diff(a: &[f32], b: &[f32]) -> (f32, f64) {
    assert_eq!(a.len(), b.len(), "長さが違う");
    let mut max = 0f32;
    let (mut e, mut r) = (0f64, 0f64);
    for (x, y) in a.iter().zip(b) {
        max = max.max((x - y).abs());
        e += ((x - y) as f64).powi(2);
        r += (*y as f64).powi(2);
    }
    (max, (e / r).sqrt())
}

fn check(name: &str, style: VoiceStyle, src: SourceGender, harmony: bool, pitch_mul: f64) {
    let input = Pcm::new(voiced(200.0, 0.6), SR);
    // 旧Kotlin版と同じレシピに固定する(現在のレシピは、より可愛い声へ調整済みで、旧版とは値が違う)
    let legacy = match (style, src) {
        (VoiceStyle::Maid, SourceGender::Male) => Recipe {
            pitch_ratio: 1.45,
            formant_ratio: 1.22,
            low_shelf_db: 0.0,
            high_shelf_db: 3.0,
        },
        (VoiceStyle::Maid, _) => Recipe {
            pitch_ratio: 1.12,
            formant_ratio: 1.06,
            low_shelf_db: 0.0,
            high_shelf_db: 3.0,
        },
        _ => recipe(style, src),
    };
    let got = render_with_recipe(&input, legacy, harmony, pitch_mul, Mode::Legacy);
    let want = golden(name);
    let (max, rel) = diff(&got.samples, &want);
    eprintln!(
        "{name}: len={} max_abs_err={max:.5} rel_rms_err={rel:.5}",
        want.len()
    );
    assert!(
        rel < 0.02,
        "{name}: Kotlin版との差が大きい (相対RMS誤差 {rel})"
    );
    assert!(max < 0.05, "{name}: 最大誤差が大きい ({max})");
}

#[test]
fn maid_female_matches_kotlin() {
    check(
        "maid_female",
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
    );
}

#[test]
fn deep_female_harmony_matches_kotlin() {
    check(
        "deep_female_harmony",
        VoiceStyle::DeepMale,
        SourceGender::Female,
        true,
        1.0,
    );
}

#[test]
fn maid_male_pitchmul_matches_kotlin() {
    check(
        "maid_male_pitchmul",
        VoiceStyle::Maid,
        SourceGender::Male,
        false,
        1.06,
    );
}

#[test]
fn deep_male_matches_kotlin() {
    check(
        "deep_male",
        VoiceStyle::DeepMale,
        SourceGender::Male,
        false,
        1.0,
    );
}
