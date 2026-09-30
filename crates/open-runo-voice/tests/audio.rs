mod common;
use common::{peak, power, rms, sine};
use open_runo_voice::dsp::{self, major_third, SourceGender};
use open_runo_voice::resampler::{resample_poly, speed_up};
use open_runo_voice::wav::{parse_wav, to_pcm16, wav_bytes, Pcm};
use open_runo_voice::VoiceStyle;
use std::f64::consts::PI;

const SR: u32 = 22050;

/// 基本周波数`f0`の倍音を持つ疑似音声(振幅ゆらぎ付き)。
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

fn mid(x: &[f32]) -> &[f32] {
    &x[SR as usize / 2..SR as usize * 3 / 2]
}

// ---- VoiceDsp ----

#[test]
fn pitch_shift_up_moves_energy_and_keeps_length() {
    let x = sine(200.0, SR, 2.0, 0.5);
    let y = dsp::pitch_shift(&x, SR, 1.5);
    assert_eq!(x.len(), y.len());
    assert!(
        power(mid(&y), SR, 300.0) > 20.0 * power(mid(&y), SR, 200.0),
        "300Hzが主成分になる"
    );
}

#[test]
fn pitch_shift_down_moves_energy() {
    let x = sine(300.0, SR, 2.0, 0.5);
    let y = dsp::pitch_shift(&x, SR, 0.8);
    assert_eq!(x.len(), y.len());
    assert!(
        power(mid(&y), SR, 240.0) > 20.0 * power(mid(&y), SR, 300.0),
        "240Hzが主成分になる"
    );
}

#[test]
fn pitch_shift_keeps_loudness_roughly() {
    let x = voiced(180.0, 1.5);
    let y = dsp::pitch_shift(&x, SR, 1.2);
    let ratio = rms(&y) / rms(&x);
    assert!((0.7..=1.3).contains(&ratio), "rms比={ratio}");
}

#[test]
fn ratio_near_one_is_identity() {
    let x = voiced(200.0, 1.5);
    assert_eq!(x, dsp::pitch_shift(&x, SR, 1.001));
}

#[test]
fn short_input_does_not_crash() {
    assert_eq!(100, dsp::pitch_shift(&vec![0.1f32; 100], SR, 1.3).len());
    assert_eq!(0, dsp::pitch_shift(&[], SR, 1.3).len());
}

#[test]
fn deep_male_from_female_source_lowers_pitch() {
    let x = sine(250.0, SR, 2.0, 0.5);
    let out = dsp::render(
        &Pcm::new(x, SR),
        VoiceStyle::DeepMale,
        SourceGender::Female,
        false,
        1.0,
    );
    let m = mid(&out.samples);
    assert!(power(m, SR, 250.0 * 0.72) > 10.0 * power(m, SR, 250.0));
}

#[test]
fn maid_raises_pitch() {
    let x = sine(250.0, SR, 2.0, 0.5);
    let out = dsp::render(
        &Pcm::new(x, SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
    );
    let m = mid(&out.samples);
    assert!(power(m, SR, 250.0 * 1.12) > 10.0 * power(m, SR, 250.0));
}

#[test]
fn harmony_contains_both_voices_a_major_third_apart() {
    let x = sine(250.0, SR, 2.0, 0.5);
    let single = dsp::render(
        &Pcm::new(x.clone(), SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
    );
    let duo = dsp::render(
        &Pcm::new(x, SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        true,
        1.0,
    );
    let fa = 250.0 * 1.12;
    let fb = fa * major_third();
    let (s, d) = (mid(&single.samples), mid(&duo.samples));
    assert!(
        power(s, SR, fb) < 0.01 * power(s, SR, fa),
        "単独には3度上の成分が無い"
    );
    assert!(
        power(d, SR, fa) > 10.0 * power(d, SR, 700.0)
            && power(d, SR, fb) > 10.0 * power(d, SR, 700.0),
        "ハモりは両方の音を含む"
    );
    assert!(power(d, SR, fb) > 0.2 * power(d, SR, fa));
    assert!(duo.samples.len() >= single.samples.len());
}

#[test]
fn output_is_normalized_without_clipping() {
    let loud = sine(200.0, SR, 1.0, 1.0);
    for harmony in [false, true] {
        for style in VoiceStyle::ALL {
            let o = dsp::render(
                &Pcm::new(loud.clone(), SR),
                style,
                SourceGender::Unknown,
                harmony,
                1.0,
            );
            let p = peak(&o.samples);
            assert!(p <= 0.91, "{style:?} harmony={harmony} peak={p}");
            assert!(p > 0.1);
        }
    }
}

#[test]
fn silence_stays_silent() {
    let o = dsp::render(
        &Pcm::new(vec![0.0; SR as usize], SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        true,
        1.0,
    );
    assert_eq!(0.0, peak(&o.samples));
}

#[test]
fn wav_round_trip() {
    let x = voiced(200.0, 0.3);
    let back = parse_wav(&wav_bytes(&Pcm::new(x.clone(), SR))).unwrap();
    assert_eq!(SR, back.sample_rate);
    assert_eq!(x.len(), back.samples.len());
    for i in (0..x.len()).step_by(97) {
        assert!((x[i] - back.samples[i]).abs() < 1e-3);
    }
}

#[test]
fn wav_rejects_garbage_and_unsupported() {
    assert!(parse_wav(&[0u8; 10]).is_none());
    assert!(parse_wav(&[0u8; 100]).is_none());
    let mut b = wav_bytes(&Pcm::new(vec![0.0; 100], SR));
    b[34] = 8; // 8bitは非対応
    assert!(parse_wav(&b).is_none());
}

#[test]
fn wav_stereo_is_averaged_and_streaming_size_tolerated() {
    // 手組みのステレオ16bit(L=+0.5, R=-0.5 → 平均0)、dataサイズ0(ストリーミング出力想定)
    let frames = 50usize;
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&SR.to_le_bytes());
    b.extend_from_slice(&(SR * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&0u32.to_le_bytes());
    for _ in 0..frames {
        b.extend_from_slice(&16384i16.to_le_bytes());
        b.extend_from_slice(&(-16384i16).to_le_bytes());
    }
    let p = parse_wav(&b).unwrap();
    assert_eq!(frames, p.samples.len());
    assert!(p.samples.iter().all(|v| v.abs() < 1e-4));
}

// ---- Resampler ----

#[test]
fn length_is_ceil_of_ratio() {
    assert_eq!(1000, resample_poly(&vec![0.0; 500], 2, 1).len());
    assert_eq!(334, resample_poly(&vec![0.0; 1000], 1, 3).len());
    assert_eq!(10, resample_poly(&[0.0; 10], 5, 5).len());
}

#[test]
fn tone_is_preserved_with_high_accuracy_when_upsampling() {
    // 1kHz@16k を 2倍(32k)に: 内部で元と同じ正弦になるはず(端の過渡部分は除く)
    let x = sine(1000.0, 16000, 4000.0 / 16000.0, 0.5);
    let y = resample_poly(&x, 2, 1);
    let refv = sine(1000.0, 32000, y.len() as f64 / 32000.0, 0.5);
    let mut max_err = 0f32;
    for i in 800..y.len() - 800 {
        max_err = max_err.max((y[i] - refv[i]).abs());
    }
    assert!(max_err < 2e-3, "maxErr={max_err}");
}

#[test]
fn above_nyquist_content_is_rejected_not_aliased() {
    // 22.05kHz系の10kHzを1.5倍速にすると15kHz(>ナイキスト11.025k)。線形補間なら折り返して残るが、ここでは消える。
    let y = speed_up(&sine(10000.0, SR, 1.0, 0.5), 1.5);
    assert!(
        rms(&y[y.len() / 4..y.len() * 3 / 4]) < 0.02,
        "折り返し雑音が残っている"
    );
    // 通過帯域(2kHz→3kHz)は保たれる
    let z = speed_up(&sine(2000.0, SR, 1.0, 0.5), 1.5);
    assert!(rms(&z[z.len() / 4..z.len() * 3 / 4]) > 0.3);
}

#[test]
fn speed_up_changes_pitch_and_length() {
    let x = sine(200.0, SR, 2.0, 0.5);
    let y = speed_up(&x, 1.25);
    assert!((x.len() as f64 / 1.25 - y.len() as f64).abs() < 3.0);
}

#[test]
fn dither_is_small_and_deterministic() {
    let x = vec![0.25f32; 1000];
    let (a, b) = (to_pcm16(&x, 1234), to_pcm16(&x, 1234));
    assert_eq!(a, b);
    let expect = (0.25f32 * 32767.0) as i32;
    assert!(a.iter().all(|v| (*v as i32 - expect).abs() <= 2));
    let distinct: std::collections::HashSet<i16> = a.iter().copied().collect();
    assert!(distinct.len() > 1, "ディザで量子化値がばらつく");
}

// ---- 磨き込み(無音トリム・音量統一・間の連結・抑揚) ----

#[test]
fn trim_silence_removes_leading_and_trailing_silence_only() {
    let body = sine(300.0, SR, 0.5, 0.5);
    let mut x = vec![0f32; SR as usize / 2];
    x.extend_from_slice(&body);
    x.extend(vec![0f32; SR as usize / 2]);
    let y = dsp::trim_silence(&x, SR, -50.0, 40);
    let expected = body.len() + 2 * (SR as usize * 40 / 1000);
    assert!(
        (y.len() as i64 - expected as i64).abs() < SR as i64 / 50,
        "size={} expected≈{expected}",
        y.len()
    );
    assert!(y.len() < x.len() / 2);
}

#[test]
fn trim_silence_keeps_inner_pause_and_all_silence() {
    let mut x = sine(300.0, SR, 0.2, 0.5);
    x.extend(vec![0f32; SR as usize / 2]);
    x.extend(sine(300.0, SR, 0.2, 0.5));
    assert_eq!(x.len(), dsp::trim_silence(&x, SR, -50.0, 40).len());
    assert_eq!(
        1000,
        dsp::trim_silence(&vec![0f32; 1000], SR, -50.0, 40).len()
    );
}

#[test]
fn loudness_normalization_equalizes_short_and_long_and_limits() {
    let a = dsp::normalize_loudness(&sine(250.0, SR, 0.4, 0.05), 0.18);
    let b = dsp::normalize_loudness(&sine(250.0, SR, 1.5, 0.9), 0.18);
    assert!(
        (rms(&a) - rms(&b)).abs() < 0.02,
        "rms {} vs {}",
        rms(&a),
        rms(&b)
    );
    // スパイク混じりでも0.9を超えない(ソフトリミッター)
    let mut spiky = sine(250.0, SR, 1.0, 0.05);
    spiky[100] = 1.0;
    spiky[5000] = -1.0;
    assert!(peak(&dsp::normalize_loudness(&spiky, 0.18)) <= 0.9);
    assert_eq!(0.0, peak(&dsp::normalize_loudness(&vec![0f32; 100], 0.18)));
}

#[test]
fn join_inserts_gaps() {
    let p1 = vec![0.5f32; SR as usize];
    let p2 = vec![0.25f32; SR as usize / 2];
    let out = dsp::join(&[p1, p2], &[500, 0], SR);
    assert_eq!(SR as usize + SR as usize / 2 + SR as usize / 2, out.len());
    assert_eq!(0.5, out[SR as usize - 1]);
    assert_eq!(0.0, out[SR as usize + 10]); // 間は無音
    assert_eq!(0.25, out[SR as usize + SR as usize / 2 + 5]);
    assert_eq!(0, dsp::join(&[], &[], SR).len());
}

#[test]
fn pitch_mul_shifts_the_segment() {
    let x = sine(250.0, SR, 2.0, 0.5);
    let base = dsp::render(
        &Pcm::new(x.clone(), SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
    );
    let up = dsp::render(
        &Pcm::new(x, SR),
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.06,
    );
    let f = 250.0 * 1.12;
    assert!(power(mid(&base.samples), SR, f) > 10.0 * power(mid(&base.samples), SR, f * 1.06));
    assert!(power(mid(&up.samples), SR, f * 1.06) > 10.0 * power(mid(&up.samples), SR, f));
}
