//! フォルマント(声の太さ・声道の大きさ)を、音程とは独立に動かす。
//!
//! 従来のリサンプリング方式は、音程を動かすと声の太さ(フォルマント)も同じ比率で動いてしまう
//! (音程を下げると声道が大きくなった声=「怪物っぽい声」になる)。ここでは、音程を動かしたあとの音声に対して、
//! **スペクトル包絡だけ**を周波数方向へ伸縮する補正を掛ける(ハーモニクスの位置=音程は変えない):
//!  1. 短時間FFTで各フレームのスペクトルを得る
//!  2. ケプストラムのリフタリングで、なめらかな包絡(対数振幅)を推定する
//!  3. 包絡を周波数軸で`ratio`倍に伸縮した「目標の包絡」との差を、フレームのスペクトルに掛ける(±[`MAX_GAIN_DB`]まで)
//!  4. 逆FFT+オーバーラップ加算で音声に戻す

use crate::fft::Fft;
use std::f64::consts::PI;

/// 補正ゲインの上限(dB)。過度な強調(ノイズの持ち上げ)を避ける。
pub const MAX_GAIN_DB: f64 = 24.0;

/// 音声のフォルマントを周波数方向へ`ratio`倍にする(音程は変えない)。`ratio>1`で声が細く/高く、`<1`で太く/低くなる。
pub fn shift_formants(x: &[f32], sr: u32, ratio: f64) -> Vec<f32> {
    if (ratio - 1.0).abs() < 0.01 || x.len() < 64 {
        return x.to_vec();
    }
    let n: usize = if sr < 32_000 { 1024 } else { 2048 };
    let hop = n / 4;
    let fft = Fft::new(n);
    let nb = n / 2 + 1;
    let win: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos())
        .collect();
    // ピッチ周期(の半分)より短いケフレンシーだけを残す=ハーモニクスの櫛を消して包絡だけにする
    let lifter = ((sr as f64 / 500.0) as usize).clamp(8, n / 8);

    let mut out = vec![0f64; x.len() + 2 * n];
    let mut norm = vec![0f64; x.len() + 2 * n];
    let mut re = vec![0f64; n];
    let mut im = vec![0f64; n];
    // 先頭・末尾の端を扱うため、フレームの開始位置は -3*hop から
    let mut start: isize = -(3 * hop as isize);
    while start < x.len() as isize {
        for i in 0..n {
            let p = start + i as isize;
            re[i] = if p >= 0 && (p as usize) < x.len() {
                x[p as usize] as f64 * win[i]
            } else {
                0.0
            };
            im[i] = 0.0;
        }
        fft.forward(&mut re, &mut im);
        let mag: Vec<f64> = (0..nb)
            .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
            .collect();
        let env = log_envelope(&mag, &fft, lifter);
        // 目標の包絡: 周波数軸を ratio 倍に伸ばす = 目標(k) = 包絡(k / ratio)
        let mut gain = vec![1f64; nb];
        for k in 0..nb {
            let src = k as f64 / ratio;
            let target = if src >= (nb - 1) as f64 {
                env[nb - 1]
            } else {
                let i0 = src as usize;
                let f = src - i0 as f64;
                env[i0] * (1.0 - f) + env[i0 + 1] * f
            };
            let db = (20.0 * (target - env[k]) / std::f64::consts::LN_10)
                .clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
            gain[k] = 10f64.powf(db / 20.0);
        }
        for k in 0..nb {
            re[k] *= gain[k];
            im[k] *= gain[k];
            if k > 0 && k < n / 2 {
                re[n - k] *= gain[k];
                im[n - k] *= gain[k];
            }
        }
        fft.inverse(&mut re, &mut im);
        for i in 0..n {
            let p = start + 3 * hop as isize + i as isize; // 出力バッファは 3*hop ずらして持つ
            if p >= 0 && (p as usize) < out.len() {
                out[p as usize] += re[i] * win[i];
                norm[p as usize] += win[i] * win[i];
            }
        }
        start += hop as isize;
    }
    (0..x.len())
        .map(|i| {
            let p = i + 3 * hop;
            if norm[p] > 1e-6 {
                (out[p] / norm[p]) as f32
            } else {
                0.0
            }
        })
        .collect()
}

/// 振幅スペクトル(0..=n/2)から、ケプストラムのリフタリングでなめらかな対数包絡(自然対数)を得る。
fn log_envelope(mag: &[f64], fft: &Fft, lifter: usize) -> Vec<f64> {
    let n = fft.len();
    let nb = n / 2 + 1;
    let mut re = vec![0f64; n];
    let mut im = vec![0f64; n];
    for k in 0..nb {
        let l = (mag[k] + 1e-9).ln();
        re[k] = l;
        if k > 0 && k < n / 2 {
            re[n - k] = l;
        }
    }
    fft.inverse(&mut re, &mut im); // ケプストラム(実数・偶対称)
    for q in 0..n {
        let d = q.min(n - q); // 対称なケフレンシー
        re[q] = if d > lifter {
            0.0
        } else if d > lifter / 2 {
            // 打ち切りの縁をなだらかに(リンギング防止)
            let t = (d - lifter / 2) as f64 / (lifter - lifter / 2) as f64;
            re[q] * (0.5 + 0.5 * (PI * t).cos())
        } else {
            re[q]
        };
        im[q] = 0.0;
    }
    fft.forward(&mut re, &mut im);
    re[..nb].to_vec()
}
