//! 音程と声の太さ(フォルマント)を独立に動かせることの検証。
//!
//! 「正解」を直接合成して比べる: 合成母音(パルス列を共鳴器3段に通したもの)は、音程`f0`とフォルマントの倍率を
//! 自由に指定して作れる。処理後の音声のスペクトル包絡が、「同じ音程・同じフォルマントで直接合成した母音」の包絡に
//! どれだけ近いかを、旧方式(音程と一緒にフォルマントも動く)と比べる。
//! (LPCでのフォルマント推定は、倍音のまばらな低い声では倍音に引っ張られて当てにならないため使わない。)

mod common;
use common::{power, rms};
use open_runo_voice::dsp::{self, pitch_shift_formant, render_with, Mode, SourceGender};
use open_runo_voice::fft::Fft;
use open_runo_voice::formant::shift_formants;
use open_runo_voice::wav::Pcm;
use open_runo_voice::VoiceStyle;
use std::f64::consts::PI;

const SR: u32 = 22050;
const N: usize = 2048;

/// 母音/a/風: 基本周波数`f0`のパルス列を、F1=700/F2=1220/F3=2600Hz(を`scale`倍)の共鳴器に通す。
fn vowel(f0: f64, seconds: f64, scale: f64) -> Vec<f32> {
    let n = (SR as f64 * seconds) as usize;
    let period = SR as f64 / f0;
    let mut x = vec![0f64; n];
    let mut next = 0.0;
    while (next as usize) < n {
        x[next as usize] = 1.0;
        next += period;
    }
    for (f, bw) in [(700.0, 80.0), (1220.0, 90.0), (2600.0, 120.0)] {
        let (f, bw) = (f * scale, bw * scale);
        let r = (-PI * bw / SR as f64).exp();
        let (a1, a2) = (2.0 * r * (2.0 * PI * f / SR as f64).cos(), -r * r);
        let gain = 1.0 - r;
        let (mut y1, mut y2) = (0.0, 0.0);
        for v in x.iter_mut() {
            let y = gain * *v + a1 * y1 + a2 * y2;
            y2 = y1;
            y1 = y;
            *v = y;
        }
    }
    let peak = x.iter().fold(0f64, |m, v| m.max(v.abs()));
    x.iter().map(|v| (v / peak * 0.5) as f32).collect()
}

/// 中央1秒の平均パワースペクトルを、`half_hz`の幅で対数領域で平滑化した包絡(dB)。10.77Hz刻みのビン。
fn envelope_db(x: &[f32], half_hz: f64) -> Vec<f64> {
    let fft = Fft::new(N);
    let win: Vec<f64> = (0..N)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / N as f64).cos())
        .collect();
    let mut acc = vec![0f64; N / 2 + 1];
    let (mut start, end) = (
        x.len() / 2 - SR as usize / 2,
        x.len() / 2 + SR as usize / 2 - N,
    );
    let mut frames = 0;
    while start < end {
        let mut re: Vec<f64> = (0..N).map(|i| x[start + i] as f64 * win[i]).collect();
        let mut im = vec![0f64; N];
        fft.forward(&mut re, &mut im);
        for k in 0..=N / 2 {
            acc[k] += re[k] * re[k] + im[k] * im[k];
        }
        frames += 1;
        start += N / 2;
    }
    let db: Vec<f64> = acc
        .iter()
        .map(|p| 10.0 * (p / frames as f64 + 1e-12).log10())
        .collect();
    let bin = SR as f64 / N as f64;
    let w = (half_hz / bin) as usize;
    (0..db.len())
        .map(|k| {
            let (lo, hi) = (k.saturating_sub(w), (k + w).min(db.len() - 1));
            db[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64
        })
        .collect()
}

/// 包絡(200〜3500Hz)の平均絶対差(dB)。全体のレベル差は引く。
fn lsd(a: &[f32], b: &[f32]) -> f64 {
    let (ea, eb) = (envelope_db(a, 200.0), envelope_db(b, 200.0));
    let bin = SR as f64 / N as f64;
    let (lo, hi) = ((200.0 / bin) as usize, (3500.0 / bin) as usize);
    let mean_diff = (lo..hi).map(|k| ea[k] - eb[k]).sum::<f64>() / (hi - lo) as f64;
    (lo..hi)
        .map(|k| (ea[k] - eb[k] - mean_diff).abs())
        .sum::<f64>()
        / (hi - lo) as f64
}

fn mid(x: &[f32]) -> &[f32] {
    &x[SR as usize / 2..SR as usize * 3 / 2]
}

#[test]
fn the_metric_tells_formant_positions_apart() {
    // 同じ音程で、フォルマントだけ違う母音どうしは、包絡が明らかに離れる(指標が使える証拠)
    let base = vowel(200.0, 1.5, 1.0);
    assert!(lsd(&base, &vowel(200.0, 1.5, 1.0)) < 0.01);
    assert!(lsd(&base, &vowel(200.0, 1.5, 0.75)) > 5.0);
}

#[test]
fn legacy_method_drags_the_formants_with_the_pitch() {
    let orig = vowel(200.0, 1.5, 1.0);
    let legacy = dsp::pitch_shift(&orig, SR, 0.72);
    // 旧方式: 音程を0.72倍にすると声の太さも0.72倍になるので、「音程144Hz・声の太さそのまま」の正解から大きく離れる
    // (=声が「怪物っぽく」なる)。フォルマントを0.72倍にした正解とは近い。
    assert!(lsd(&legacy, &vowel(144.0, 1.5, 1.0)) > 6.0);
    assert!(lsd(&legacy, &vowel(144.0, 1.5, 0.72)) < 3.0);
}

#[test]
fn pitch_goes_down_but_the_formants_stay_when_asked() {
    let orig = vowel(200.0, 1.5, 1.0);
    let legacy = dsp::pitch_shift(&orig, SR, 0.72);
    let new = pitch_shift_formant(&orig, SR, 0.72, 1.0);
    let gt = vowel(144.0, 1.5, 1.0); // 音程144Hz・声の太さは元のまま、を直接合成した正解
                                     // 音程: 基本周波数が144Hzになり、元の200Hzは消える
    assert!(
        power(mid(&new), SR, 144.0) > 20.0 * power(mid(&new), SR, 200.0),
        "音程が下がっていない"
    );
    // 声の太さ: 正解の包絡に近い(旧方式よりずっと)
    let (d_new, d_old) = (lsd(&new, &gt), lsd(&legacy, &gt));
    assert!(d_new < 3.0, "新方式の包絡が正解から遠い: {d_new}dB");
    assert!(d_new < 0.4 * d_old, "新={d_new}dB 旧={d_old}dB");
}

#[test]
fn formants_move_independently_of_the_pitch() {
    let orig = vowel(200.0, 1.5, 1.0);
    for scale in [1.25, 0.8] {
        let y = pitch_shift_formant(&orig, SR, 1.0, scale); // 音程はそのまま、声の太さだけ動かす
        let gt = vowel(200.0, 1.5, scale);
        // 音程は動かない(200Hzのまま)
        assert!(
            power(mid(&y), SR, 200.0)
                > 20.0 * power(mid(&y), SR, if scale > 1.0 { 250.0 } else { 160.0 }),
            "scale={scale}: 音程が動いた"
        );
        let (d, d0) = (lsd(&y, &gt), lsd(&orig, &gt));
        assert!(d < 3.0, "scale={scale}: 包絡が正解から遠い: {d}dB");
        assert!(d < 0.5 * d0, "scale={scale}: 処理後 {d}dB / 未処理 {d0}dB");
    }
}

#[test]
fn ratio_near_one_returns_the_input_and_length_is_kept() {
    let x = vowel(200.0, 1.0, 1.0);
    assert_eq!(x, shift_formants(&x, SR, 1.005));
    assert_eq!(x.len(), shift_formants(&x, SR, 1.3).len());
    assert_eq!(10, shift_formants(&[0.1; 10], SR, 1.3).len()); // 短すぎる入力は素通し
}

#[test]
fn formant_shift_keeps_loudness_and_does_not_blow_up() {
    let x = vowel(200.0, 1.5, 1.0);
    for ratio in [0.7, 0.85, 1.2, 1.5] {
        let y = shift_formants(&x, SR, ratio);
        let r = rms(&y) / rms(&x);
        assert!((0.3..3.0).contains(&r), "ratio={ratio}: rms比={r}");
        assert!(
            y.iter().all(|v| v.is_finite() && v.abs() < 4.0),
            "ratio={ratio}: 発散"
        );
    }
}

/// 既定の新方式の「太い男性」(女性の声のもとから)。音程は旧方式と同じだけ下がり、声の太さはレシピの目標
/// (0.82倍)に近づく。旧方式は音程と同じ0.72倍まで下がってしまう。
#[test]
fn new_default_deep_voice_hits_its_target_formants() {
    let pcm = Pcm::new(vowel(250.0, 1.5, 1.0), SR);
    let old = render_with(
        &pcm,
        VoiceStyle::DeepMale,
        SourceGender::Female,
        false,
        1.0,
        Mode::Legacy,
    );
    let new = render_with(
        &pcm,
        VoiceStyle::DeepMale,
        SourceGender::Female,
        false,
        1.0,
        Mode::FormantIndependent,
    );
    for p in [&old, &new] {
        assert!(
            power(mid(&p.samples), SR, 180.0) > 10.0 * power(mid(&p.samples), SR, 250.0),
            "音程が180Hzになっていない"
        );
    }
    // 目標: 音程180Hz・声の太さ0.82倍 の母音。(このあとのEQで包絡が多少変わるので、旧方式との比較で見る)
    let gt = vowel(180.0, 1.5, 0.82);
    let (d_new, d_old) = (lsd(&new.samples, &gt), lsd(&old.samples, &gt));
    assert!(d_new < d_old, "新={d_new}dB 旧={d_old}dB");
}

/// 「メイド風」(女性の声のもとから)も同様。音程1.12倍・声の太さ1.06倍。
#[test]
fn new_default_maid_voice_hits_its_target_formants() {
    let pcm = Pcm::new(vowel(250.0, 1.5, 1.0), SR);
    let old = render_with(
        &pcm,
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
        Mode::Legacy,
    );
    let new = render_with(
        &pcm,
        VoiceStyle::Maid,
        SourceGender::Female,
        false,
        1.0,
        Mode::FormantIndependent,
    );
    let gt = vowel(280.0, 1.5, 1.06);
    let (d_new, d_old) = (lsd(&new.samples, &gt), lsd(&old.samples, &gt));
    assert!(d_new <= d_old + 0.3, "新={d_new}dB 旧={d_old}dB"); // 補正が小さい(1.06/1.12)ので、悪化しないことを確認
}
