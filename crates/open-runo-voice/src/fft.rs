//! 小さな基数2のFFT(依存クレート無し)。フォルマント制御のスペクトル分析用。

use std::f64::consts::PI;

/// 長さ`n`(2のべき乗)の複素FFT。実部/虚部を別配列で持つ。
#[derive(Debug)]
pub struct Fft {
    n: usize,
    cos: Vec<f64>,
    sin: Vec<f64>,
    rev: Vec<usize>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2, "FFT長は2のべき乗: {n}");
        let bits = n.trailing_zeros();
        let rev = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let cos = (0..n / 2)
            .map(|k| (2.0 * PI * k as f64 / n as f64).cos())
            .collect();
        let sin = (0..n / 2)
            .map(|k| (2.0 * PI * k as f64 / n as f64).sin())
            .collect();
        Fft { n, cos, sin, rev }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    fn run(&self, re: &mut [f64], im: &mut [f64], sign: f64) {
        assert!(re.len() == self.n && im.len() == self.n);
        for i in 0..self.n {
            let j = self.rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= self.n {
            let half = len / 2;
            let step = self.n / len;
            for start in (0..self.n).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], sign * self.sin[k * step]);
                    let (a, b) = (start + k, start + k + half);
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }

    /// 順変換(スケーリング無し)。
    pub fn forward(&self, re: &mut [f64], im: &mut [f64]) {
        self.run(re, im, -1.0);
    }

    /// 逆変換(1/nでスケーリング済み)。
    pub fn inverse(&self, re: &mut [f64], im: &mut [f64]) {
        self.run(re, im, 1.0);
        let s = 1.0 / self.n as f64;
        for v in re.iter_mut().chain(im.iter_mut()) {
            *v *= s;
        }
    }
}
