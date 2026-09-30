#![allow(dead_code)]
//! テスト共通の道具(音声の解析用)。

/// Goertzel法で周波数`f`の成分強度。
pub fn power(x: &[f32], sr: u32, f: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * f / sr as f64;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0f64, 0f64);
    for v in x {
        let s = *v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    (s1 * s1 + s2 * s2 - c * s1 * s2) / (x.len() as f64 * x.len() as f64)
}

pub fn sine(f: f64, sr: u32, seconds: f64, amp: f32) -> Vec<f32> {
    (0..(sr as f64 * seconds) as usize)
        .map(|i| (2.0 * std::f64::consts::PI * f * i as f64 / sr as f64).sin() as f32 * amp)
        .collect()
}

pub fn rms(a: &[f32]) -> f64 {
    (a.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / a.len() as f64).sqrt()
}

pub fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0f32, |m, v| m.max(v.abs()))
}
