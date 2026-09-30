//! 高品質リサンプラ。make-disk(`engine/audio_sr.rs`の`resample_poly`、scipy互換で精度検証済み)と同じ設計:
//! Kaiser窓(β=5)のFIR、半長は10×max(up,down)、多相実装。
//! 線形補間と違い、間引き時の折り返し雑音・補間時のイメージングが出ない。

use std::f64::consts::PI;

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)) * (x / (2.0 * k));
        sum += term;
        k += 1.0;
    }
    sum
}

/// 出力長は ceil(n*up/down)。`up==down`ならコピー。
pub fn resample_poly(x: &[f32], up_in: usize, down_in: usize) -> Vec<f32> {
    assert!(up_in > 0 && down_in > 0);
    if up_in == down_in || x.is_empty() {
        return x.to_vec();
    }
    let g = gcd(up_in, down_in);
    let (up, down) = (up_in / g, down_in / g);
    let half = 10 * up.max(down);
    let len = 2 * half + 1;
    let cutoff = 1.0 / up.max(down) as f64;
    let beta = 5.0;
    let i0b = bessel_i0(beta);
    let hd: Vec<f64> = (0..len)
        .map(|n| {
            let t = n as f64 - half as f64;
            let sinc = if t == 0.0 {
                1.0
            } else {
                (PI * cutoff * t).sin() / (PI * cutoff * t)
            };
            let r = 2.0 * n as f64 / (len - 1) as f64 - 1.0;
            cutoff * sinc * bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0b
        })
        .collect();
    let sum: f64 = hd.iter().sum();
    let h: Vec<f32> = hd.iter().map(|v| (v * up as f64 / sum) as f32).collect();

    let out_len = (x.len() * up).div_ceil(down);
    let mut y = vec![0f32; out_len];
    for (m, out) in y.iter_mut().enumerate() {
        let base = m * down + half; // アップサンプル後の系列でのタップ中心
        let k_min = base.saturating_sub(2 * half).div_ceil(up);
        let k_max = (base / up).min(x.len() - 1);
        let mut acc = 0f32;
        for k in k_min..=k_max {
            acc += x[k] * h[base - k * up];
        }
        *out = acc;
    }
    y
}

/// 再生速度を`ratio`倍にした結果(長さ1/ratio・音程ratio倍)。比率は1/200刻みに丸める(3度の誤差は4セント以内)。
pub fn speed_up(x: &[f32], ratio: f64) -> Vec<f32> {
    let denom = 200usize;
    let down = ((ratio * denom as f64 + 0.5).floor() as i64).max(1) as usize;
    resample_poly(x, denom, down)
}
