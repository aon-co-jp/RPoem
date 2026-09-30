//! 音声処理の速度の目安(`cargo test --release --test bench -- --nocapture --ignored`)。通常のテストでは走らない。

use open_runo_voice::dsp::{render, SourceGender};
use open_runo_voice::wav::Pcm;
use open_runo_voice::VoiceStyle;
use std::f64::consts::PI;
use std::time::Instant;

#[test]
#[ignore]
fn render_speed() {
    let sr = 22050u32;
    let secs = 4.0;
    let x: Vec<f32> = (0..(sr as f64 * secs) as usize)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let env = 0.6 + 0.4 * (2.0 * PI * 3.0 * t).sin();
            let v: f64 = (1..=5)
                .map(|h| (2.0 * PI * 200.0 * h as f64 * t).sin() / h as f64)
                .sum();
            (env * v * 0.3) as f32
        })
        .collect();
    let input = Pcm::new(x, sr);
    for (name, harmony) in [("単独", false), ("ハモり", true)] {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = Instant::now();
            let _ = render(&input, VoiceStyle::Maid, SourceGender::Female, harmony, 1.0);
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!("{secs}秒の音声を加工({name}): {best:.0} ms(最速)");
    }
}
