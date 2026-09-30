//! モノラルPCMと、16bit PCM WAVの読み書き(TTSエンジンの出力を読むための最小実装)。

/// モノラルのPCM(-1..1)。
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Pcm {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Pcm {
            samples,
            sample_rate,
        }
    }

    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}

/// `java.util.Random`と同じ48bit線形合同法。ディザの系列をKotlin版と一致させ、再現可能にする。
#[derive(Debug)]
pub struct JavaRandom {
    seed: u64,
}

const MASK48: u64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        JavaRandom {
            seed: (seed as u64 ^ 0x5DEECE66D) & MASK48,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(0x5DEECE66D).wrapping_add(0xB) & MASK48;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1u32 << 24) as f32
    }
}

/// 16bit化。量子化歪みを聴こえにくくするため三角分布(TPDF)ディザを加える
/// (make-diskの`triangular_hp`ディザと同じ考え方)。乱数は固定シード(既定1234)で再現可能。
pub fn to_pcm16(x: &[f32], seed: i64) -> Vec<i16> {
    let mut rnd = JavaRandom::new(seed);
    x.iter()
        .map(|&v| {
            let a = rnd.next_float();
            let b = rnd.next_float();
            let d = (a - b) / 32768.0;
            let s = ((v + d).clamp(-1.0, 1.0) * 32767.0 + 0.5).floor(); // Kotlinのroundは「.5は切り上げ」
            s as i16
        })
        .collect()
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn i16_at(b: &[u8], i: usize) -> i16 {
    i16::from_le_bytes([b[i], b[i + 1]])
}
fn i32_at(b: &[u8], i: usize) -> i32 {
    i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// 16bit PCM(モノラル/ステレオ)を読む。ステレオはモノラルに平均。非対応形式・破損は`None`。
pub fn parse_wav(bytes: &[u8]) -> Option<Pcm> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut channels, mut rate, mut bits, mut format) = (0i32, 0i32, 0i32, 0u32);
    let mut pos: usize = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let mut size = i32_at(bytes, pos + 4);
        let body = pos + 8;
        if id == b"fmt " {
            if size < 16 || body + 16 > bytes.len() {
                return None;
            }
            format = u16_at(bytes, body) as u32;
            channels = i16_at(bytes, body + 2) as i32;
            rate = i32_at(bytes, body + 4);
            bits = i16_at(bytes, body + 14) as i32;
        } else if id == b"data" {
            if (format != 1 && format != 0xFFFE)
                || bits != 16
                || !(1..=2).contains(&channels)
                || rate <= 0
            {
                return None;
            }
            // ストリーミング出力でサイズが0や過大になっている場合はファイル末尾までとみなす
            if size <= 0 || body as i64 + size as i64 > bytes.len() as i64 {
                size = (bytes.len() - body) as i32;
            }
            let ch = channels as usize;
            let frames = size as usize / (2 * ch);
            let mut out = Vec::with_capacity(frames);
            for i in 0..frames {
                let mut sum = 0f32;
                for c in 0..ch {
                    sum += i16_at(bytes, body + (i * ch + c) * 2) as f32 / 32768.0;
                }
                out.push(sum / ch as f32);
            }
            return Some(Pcm::new(out, rate as u32));
        }
        if size < 0 {
            return None;
        }
        pos = body + size as usize + (size as usize & 1);
    }
    None
}

/// 16bit・モノラルのWAVにする(ディザつき)。
pub fn wav_bytes(p: &Pcm) -> Vec<u8> {
    let data = to_pcm16(&p.samples, 1234);
    let n = data.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + n as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&p.sample_rate.to_le_bytes());
    b.extend_from_slice(&(p.sample_rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&n.to_le_bytes());
    for s in data {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}
