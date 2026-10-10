//! CPU で動く AI 超解像(Real-ESRGAN `realesr-general-x4v3` などの小型の畳み込みネット)。
//!
//! `make-disk`(aon-co-jp/make-disk)で作った CPU 版を、共有クレートへ切り出したもの。公式の ncnn 形式のモデル
//! (`.param`/`.bin`)を自前で読み込み、SRVGGNetCompact(3x3 畳み込み×18層 + PReLU + PixelShuffle + 最近傍拡大の加算)を
//! CPU で推論する。x86_64 で AVX2+FMA が使えれば専用カーネルを、無ければスカラー実装を使う(Arm は現状スカラーで遅い)。
//!
//! ## 設計
//! - 画像を128×128のタイルに分け、各タイルに受容野の半径(=畳み込み層数18)だけ余白を付けて処理するので、
//!   タイル境界でも画像全体を一度に処理した結果と(浮動小数点の誤差を除いて)一致する。タイルは複数スレッドで並列処理。
//! - 活性化はHWC(チャンネルが連続)。重みは`[tap][入力ch][出力ch]`に並べ替え、AVX2では出力16ch×4画素を
//!   レジスタに保持して重みの読み込みを再利用する。
//!
//! **正直な開示**: 対応モデルは SRVGGNetCompact 系(`realesr-general-x4v3`・`realesr-animevideov3`)のみ。高品質な `realesrgan-x4plus`
//! (RRDBNet、約64MB)は構造が大きく異なり、CPU では非現実的なため対象外。速度はコア数・命令セット次第。



const FP16_FLAG: u32 = 0x0130_6B47;
/// 3x3畳み込みを18層重ねた受容野の半径。タイルの余白にする。
const HALO: usize = 18;
const TILE: usize = 128;

#[derive(Debug)]
struct ConvLayer {
    in_c: usize,
    out_c: usize,
    /// `[tap(0..9)][in_c][out_c]`
    w: Vec<f32>,
    bias: Vec<f32>,
    prelu: Option<Vec<f32>>,
}

#[derive(Debug)]
pub struct SrModel {
    layers: Vec<ConvLayer>,
    /// ネットワーク自体の拡大倍率(PixelShuffleの倍率、常に4)。
    scale: usize,
    /// 後段の双三次リサイズ倍率(x2は0.5、x3は0.75。x4はNone)。
    post_resize: Option<f32>,
}

impl SrModel {
    /// x4専用のネットワーク(例: `realesr-general-x4v3`)から、2倍・3倍の出力を作るための後段リサイズを設定する
    /// (ncnnの公式配布のx2/x3モデルが、x4の出力を双三次で縮小しているのと同じ方法)。`target`が4以上なら何もしない。
    pub fn with_output_scale(mut self, target: u32) -> Self {
        if (2..self.scale as u32).contains(&target) {
            self.post_resize = Some(target as f32 / self.scale as f32);
        }
        self
    }

    /// ネットワークの拡大倍率(後段のリサイズ前)。
    pub fn scale(&self) -> usize {
        self.scale
    }

    /// 入力`w`×`h`に対する最終的な出力サイズ。
    pub fn output_size(&self, w: usize, h: usize) -> (usize, usize) {
        let f = self.post_resize.unwrap_or(1.0) * self.scale as f32;
        (((w as f32) * f).round() as usize, ((h as f32) * f).round() as usize)
    }
}


fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1f) as u32;
    let frac = (h & 0x3ff) as u32;
    let bits = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            // 非正規化数
            let mut e = 127 - 15 + 1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            (sign << 31) | ((e as u32) << 23) | ((f & 0x3ff) << 13)
        }
    } else if exp == 31 {
        (sign << 31) | 0x7f80_0000 | (frac << 13)
    } else {
        (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13)
    };
    f32::from_bits(bits)
}

struct BinReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl BinReader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or("モデルの.binが途中で終わっています")?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// タグ付きの重み(fp16またはfloat32)を読む。fp16は4バイト境界まで読み飛ばす。
    fn tagged_weights(&mut self, n: usize) -> Result<Vec<f32>, String> {
        let flag = self.u32()?;
        match flag {
            FP16_FLAG => {
                let raw = self.take(n * 2)?.to_vec();
                let out = raw.as_chunks::<2>().0.iter().map(|c| half_to_f32(u16::from_le_bytes(*c))).collect();
                self.pos = (self.pos + 3) & !3;
                Ok(out)
            }
            0 => self.floats(n),
            other => Err(format!("未対応の重み形式です(flag=0x{other:08x})")),
        }
    }
    fn floats(&mut self, n: usize) -> Result<Vec<f32>, String> {
        Ok(self.take(n * 4)?.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect())
    }
}

/// ncnnの`.param`(テキスト)と`.bin`から、対応する構造のモデルを読み込む。
pub fn parse_model(param: &str, bin: &[u8]) -> Result<SrModel, String> {
    let mut lines = param.lines();
    if lines.next().map(str::trim) != Some("7767517") {
        return Err("ncnnの.paramではありません(マジック不一致)".to_string());
    }
    lines.next(); // レイヤー数/blob数
    let mut reader = BinReader { data: bin, pos: 0 };
    let mut layers: Vec<ConvLayer> = Vec::new();
    let mut shuffle: Option<usize> = None;
    let mut interp: Option<(i32, f32)> = None;
    let mut added = false;
    let mut post_resize: Option<f32> = None;
    let mut prev_in_c = 3usize;

    for line in lines.filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 4 {
            return Err(format!("解析できない行です: {line}"));
        }
        let n_in: usize = t[2].parse().map_err(|_| "入力数が不正です")?;
        let n_out: usize = t[3].parse().map_err(|_| "出力数が不正です")?;
        let kv: std::collections::HashMap<&str, &str> = t[4 + n_in + n_out..].iter().filter_map(|s| s.split_once('=')).collect();
        let geti = |k: &str| -> Option<i64> { kv.get(k).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64) };
        match t[0] {
            "Input" | "Split" => {}
            "Convolution" => {
                let out_c = geti("0").ok_or("出力チャンネル数がありません")? as usize;
                let (k, pad, bias_term, wsize) = (geti("1").unwrap_or(0), geti("4").unwrap_or(0), geti("5").unwrap_or(0), geti("6").ok_or("重みサイズがありません")? as usize);
                if k != 3 || pad != 1 || bias_term != 1 {
                    return Err("未対応の畳み込みです(3x3・パディング1・バイアス有りのみ対応)".to_string());
                }
                if out_c == 0 || wsize % (out_c * 9) != 0 {
                    return Err("重みサイズが不正です".to_string());
                }
                let in_c = wsize / (out_c * 9);
                if in_c != prev_in_c {
                    return Err(format!("層のチャンネル数がつながりません(期待{prev_in_c}、実際{in_c})"));
                }
                let raw = reader.tagged_weights(wsize)?; // [oc][ic][ky][kx]
                let bias = reader.floats(out_c)?;
                let mut w = vec![0f32; wsize];
                for oc in 0..out_c {
                    for ic in 0..in_c {
                        for tap in 0..9 {
                            w[(tap * in_c + ic) * out_c + oc] = raw[(oc * in_c + ic) * 9 + tap];
                        }
                    }
                }
                layers.push(ConvLayer { in_c, out_c, w, bias, prelu: None });
                prev_in_c = out_c;
            }
            "PReLU" => {
                let n = geti("0").ok_or("PReLUのチャンネル数がありません")? as usize;
                let slopes = reader.floats(n)?;
                let last = layers.last_mut().ok_or("PReLUの前に畳み込みがありません")?;
                if last.out_c != n {
                    return Err("PReLUのチャンネル数が畳み込みと一致しません".to_string());
                }
                last.prelu = Some(slopes);
            }
            "PixelShuffle" => shuffle = Some(geti("0").ok_or("PixelShuffleの倍率がありません")? as usize),
            "Interp" => {
                let mode = geti("0").unwrap_or(0) as i32;
                let factor: f32 = kv.get("1").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                if added {
                    // 加算の後のInterpは、x2/x3モデルの最終リサイズ(双三次、mode 3)。
                    if mode != 3 || !(0.1..=1.0).contains(&factor) {
                        return Err("未対応の後段リサイズです(双三次の縮小のみ対応)".to_string());
                    }
                    post_resize = Some(factor);
                } else {
                    interp = Some((mode, factor));
                }
            }
            "BinaryOp" => {
                if geti("0").unwrap_or(0) != 0 {
                    return Err("未対応のBinaryOpです(加算のみ対応)".to_string());
                }
                added = true;
            }
            other => return Err(format!("未対応の層です: {other}(realesr-animevideov3のみ対応)")),
        }
    }
    let scale = shuffle.ok_or("PixelShuffleがありません")?;
    let (mode, factor) = interp.ok_or("Interpがありません")?;
    if mode != 1 || (factor - scale as f32).abs() > 1e-3 || !added {
        return Err("未対応の構造です(最近傍拡大+加算の残差接続のみ対応)".to_string());
    }
    if layers.last().map(|l| l.out_c) != Some(3 * scale * scale) {
        return Err("最終層の出力チャンネル数がPixelShuffleと合いません".to_string());
    }
    if reader.pos != bin.len() {
        return Err(format!("重みファイルに未使用のデータが残っています(読了{} / {}バイト)", reader.pos, bin.len()));
    }
    Ok(SrModel { layers, scale, post_resize })
}



/// 使う計算カーネルの名前(ログ表示用)。open-cpuの検出結果に従う。
pub fn kernel_name() -> &'static str {
    if avx2_available() {
        "AVX2+FMA"
    } else if cfg!(target_arch = "aarch64") {
        "NEON"
    } else {
        "scalar"
    }
}

fn avx2_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// ゼロ余白付きのHWCバッファ`(h+2)*(w+2)*c`。
fn padded(h: usize, w: usize, c: usize) -> Vec<f32> {
    vec![0f32; (h + 2) * (w + 2) * c]
}

fn activate(v: f32, slope: Option<f32>) -> f32 {
    match slope {
        Some(s) => v.max(0.0) + s * v.min(0.0),
        None => v,
    }
}

/// スカラー(可搬)実装。AVX2カーネルの正解参照にもなる。
fn conv_generic(inp: &[f32], out: &mut [f32], h: usize, w: usize, l: &ConvLayer) {
    let (ic_n, oc_n) = (l.in_c, l.out_c);
    let (sin, sout) = ((w + 2) * ic_n, (w + 2) * oc_n);
    let mut acc = vec![0f32; oc_n];
    for y in 0..h {
        for x in 0..w {
            acc.copy_from_slice(&l.bias);
            for tap in 0..9 {
                let (ky, kx) = (tap / 3, tap % 3);
                let px = &inp[(y + ky) * sin + (x + kx) * ic_n..][..ic_n];
                for (ic, &v) in px.iter().enumerate() {
                    let wrow = &l.w[(tap * ic_n + ic) * oc_n..][..oc_n];
                    for (a, &wv) in acc.iter_mut().zip(wrow) {
                        *a += v * wv;
                    }
                }
            }
            let dst = &mut out[(y + 1) * sout + (x + 1) * oc_n..][..oc_n];
            for (o, d) in dst.iter_mut().enumerate() {
                *d = activate(acc[o], l.prelu.as_ref().map(|p| p[o]));
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn conv_avx2(inp: &[f32], out: &mut [f32], h: usize, w: usize, l: &ConvLayer) {
    use std::arch::x86_64::*;
    let (ic_n, oc_n) = (l.in_c, l.out_c);
    let (sin, sout) = ((w + 2) * ic_n, (w + 2) * oc_n);
    let zero = _mm256_setzero_ps();
    for y in 0..h {
        let mut ob = 0;
        while ob < oc_n {
            let b0 = _mm256_loadu_ps(l.bias.as_ptr().add(ob));
            let b1 = _mm256_loadu_ps(l.bias.as_ptr().add(ob + 8));
            let (s0, s1) = match &l.prelu {
                Some(p) => (_mm256_loadu_ps(p.as_ptr().add(ob)), _mm256_loadu_ps(p.as_ptr().add(ob + 8))),
                None => (zero, zero),
            };
            let mut x = 0;
            // 4画素ずつ: 出力16ch×4画素=8個のアキュムレータをレジスタに保持する。
            while x + 4 <= w {
                let mut a = [[b0, b1]; 4];
                for tap in 0..9 {
                    let (ky, kx) = (tap / 3, tap % 3);
                    let base = inp.as_ptr().add((y + ky) * sin + (x + kx) * ic_n);
                    let wp = l.w.as_ptr().add(tap * ic_n * oc_n + ob);
                    for ic in 0..ic_n {
                        let w0 = _mm256_loadu_ps(wp.add(ic * oc_n));
                        let w1 = _mm256_loadu_ps(wp.add(ic * oc_n + 8));
                        for (p, ap) in a.iter_mut().enumerate() {
                            let v = _mm256_broadcast_ss(&*base.add(p * ic_n + ic));
                            ap[0] = _mm256_fmadd_ps(v, w0, ap[0]);
                            ap[1] = _mm256_fmadd_ps(v, w1, ap[1]);
                        }
                    }
                }
                for (p, ap) in a.iter().enumerate() {
                    let dst = out.as_mut_ptr().add((y + 1) * sout + (x + p + 1) * oc_n + ob);
                    for (j, (acc, s)) in [(ap[0], s0), (ap[1], s1)].into_iter().enumerate() {
                        let r = if l.prelu.is_some() { _mm256_fmadd_ps(s, _mm256_min_ps(acc, zero), _mm256_max_ps(acc, zero)) } else { acc };
                        _mm256_storeu_ps(dst.add(j * 8), r);
                    }
                }
                x += 4;
            }
            // 余りの画素(1画素ずつ)
            while x < w {
                let (mut a0, mut a1) = (b0, b1);
                for tap in 0..9 {
                    let (ky, kx) = (tap / 3, tap % 3);
                    let base = inp.as_ptr().add((y + ky) * sin + (x + kx) * ic_n);
                    let wp = l.w.as_ptr().add(tap * ic_n * oc_n + ob);
                    for ic in 0..ic_n {
                        let v = _mm256_broadcast_ss(&*base.add(ic));
                        a0 = _mm256_fmadd_ps(v, _mm256_loadu_ps(wp.add(ic * oc_n)), a0);
                        a1 = _mm256_fmadd_ps(v, _mm256_loadu_ps(wp.add(ic * oc_n + 8)), a1);
                    }
                }
                let dst = out.as_mut_ptr().add((y + 1) * sout + (x + 1) * oc_n + ob);
                for (j, (acc, s)) in [(a0, s0), (a1, s1)].into_iter().enumerate() {
                    let r = if l.prelu.is_some() { _mm256_fmadd_ps(s, _mm256_min_ps(acc, zero), _mm256_max_ps(acc, zero)) } else { acc };
                    _mm256_storeu_ps(dst.add(j * 8), r);
                }
                x += 1;
            }
            ob += 16;
        }
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn conv_neon(inp: &[f32], out: &mut [f32], h: usize, w: usize, l: &ConvLayer) {
    use std::arch::aarch64::*;
    let (ic_n, oc_n) = (l.in_c, l.out_c);
    let (sin, sout) = ((w + 2) * ic_n, (w + 2) * oc_n);
    let zero = vdupq_n_f32(0.0);
    for y in 0..h {
        let mut ob = 0;
        while ob < oc_n {
            let mut b = [zero; 4];
            let mut sl = [zero; 4];
            for j in 0..4 {
                b[j] = vld1q_f32(l.bias.as_ptr().add(ob + 4 * j));
                if let Some(p) = &l.prelu {
                    sl[j] = vld1q_f32(p.as_ptr().add(ob + 4 * j));
                }
            }
            let mut x = 0;
            // 2画素ずつ: 出力16ch×2画素=8個のアキュムレータ。
            while x + 2 <= w {
                let mut a = [b, b];
                for tap in 0..9 {
                    let (ky, kx) = (tap / 3, tap % 3);
                    let base = inp.as_ptr().add((y + ky) * sin + (x + kx) * ic_n);
                    let wp = l.w.as_ptr().add(tap * ic_n * oc_n + ob);
                    for ic in 0..ic_n {
                        let wv = [
                            vld1q_f32(wp.add(ic * oc_n)),
                            vld1q_f32(wp.add(ic * oc_n + 4)),
                            vld1q_f32(wp.add(ic * oc_n + 8)),
                            vld1q_f32(wp.add(ic * oc_n + 12)),
                        ];
                        for (p, ap) in a.iter_mut().enumerate() {
                            let v = *base.add(p * ic_n + ic);
                            for j in 0..4 {
                                ap[j] = vfmaq_n_f32(ap[j], wv[j], v);
                            }
                        }
                    }
                }
                for (p, ap) in a.iter().enumerate() {
                    let dst = out.as_mut_ptr().add((y + 1) * sout + (x + p + 1) * oc_n + ob);
                    for j in 0..4 {
                        let acc = ap[j];
                        let r = if l.prelu.is_some() { vfmaq_f32(vmaxq_f32(acc, zero), sl[j], vminq_f32(acc, zero)) } else { acc };
                        vst1q_f32(dst.add(4 * j), r);
                    }
                }
                x += 2;
            }
            while x < w {
                let mut a = b;
                for tap in 0..9 {
                    let (ky, kx) = (tap / 3, tap % 3);
                    let base = inp.as_ptr().add((y + ky) * sin + (x + kx) * ic_n);
                    let wp = l.w.as_ptr().add(tap * ic_n * oc_n + ob);
                    for ic in 0..ic_n {
                        let v = *base.add(ic);
                        for j in 0..4 {
                            a[j] = vfmaq_n_f32(a[j], vld1q_f32(wp.add(ic * oc_n + 4 * j)), v);
                        }
                    }
                }
                let dst = out.as_mut_ptr().add((y + 1) * sout + (x + 1) * oc_n + ob);
                for j in 0..4 {
                    let acc = a[j];
                    let r = if l.prelu.is_some() { vfmaq_f32(vmaxq_f32(acc, zero), sl[j], vminq_f32(acc, zero)) } else { acc };
                    vst1q_f32(dst.add(4 * j), r);
                }
                x += 1;
            }
            ob += 16;
        }
    }
}

fn conv_layer(inp: &[f32], out: &mut [f32], h: usize, w: usize, l: &ConvLayer, use_avx2: bool) {
    #[cfg(target_arch = "x86_64")]
    if use_avx2 && l.out_c.is_multiple_of(16) {
        // 安全性: avx2+fmaはavx2_available()(open-cpu)で確認済み。範囲はバッファ寸法から決まる。
        unsafe { conv_avx2(inp, out, h, w, l) };
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if l.out_c.is_multiple_of(16) {
        // 安全性: NEON は aarch64 の基本機能。範囲はバッファ寸法から決まる。
        unsafe { conv_neon(inp, out, h, w, l) };
        return;
    }
    let _ = use_avx2;
    conv_generic(inp, out, h, w, l);
}

/// 1タイル(HWC、`h`×`w`×3、0〜1)を処理して`(h*r)×(w*r)×3`(0〜1、クリップ済み)を返す。
fn run_tile(model: &SrModel, rgb: &[f32], h: usize, w: usize, use_avx2: bool) -> Vec<f32> {
    let mut cur = padded(h, w, 3);
    for y in 0..h {
        for x in 0..w {
            cur[(y + 1) * (w + 2) * 3 + (x + 1) * 3..][..3].copy_from_slice(&rgb[(y * w + x) * 3..][..3]);
        }
    }
    for l in &model.layers {
        let mut next = padded(h, w, l.out_c);
        conv_layer(&cur, &mut next, h, w, l, use_avx2);
        cur = next;
    }
    let r = model.scale;
    let sc = (w + 2) * model.layers.last().map_or(0, |l| l.out_c);
    let oc = model.layers.last().map_or(0, |l| l.out_c);
    let mut out = vec![0f32; h * r * w * r * 3];
    for y in 0..h {
        for x in 0..w {
            let feat = &cur[(y + 1) * sc + (x + 1) * oc..][..oc];
            for c in 0..3 {
                let base_in = rgb[(y * w + x) * 3 + c];
                for i in 0..r {
                    for j in 0..r {
                        // PixelShuffle: out[c][y*r+i][x*r+j] = in[c*r*r + i*r + j][y][x]
                        let v = feat[c * r * r + i * r + j] + base_in;
                        out[((y * r + i) * (w * r) + (x * r + j)) * 3 + c] = v.clamp(0.0, 1.0);
                    }
                }
            }
        }
    }
    out
}


/// `upscale_rgb`のスレッド数指定版(`0`=全論理コア)。GPUと同時に動かすとき、GPU側の読み書きに数コアを残すために使う。
pub(crate) fn upscale_rgb_threads(model: &SrModel, rgb: &[f32], w: usize, h: usize, threads: usize) -> Vec<f32> {
    let r = model.scale;
    let use_avx2 = avx2_available();
    let mut tiles = Vec::new();
    for ty in (0..h).step_by(TILE) {
        for tx in (0..w).step_by(TILE) {
            tiles.push((tx, ty));
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads = (if threads == 0 { cores } else { threads.min(cores) }).max(1).min(tiles.len().max(1));
    let results = std::sync::Mutex::new(Vec::<(usize, usize, usize, usize, Vec<f32>)>::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(tx, ty)) = tiles.get(i) else { break };
                let (x1, y1) = ((tx + TILE).min(w), (ty + TILE).min(h));
                let (rx0, ry0) = (tx.saturating_sub(HALO), ty.saturating_sub(HALO));
                let (rx1, ry1) = ((x1 + HALO).min(w), (y1 + HALO).min(h));
                let (rw, rh) = (rx1 - rx0, ry1 - ry0);
                let mut region = vec![0f32; rw * rh * 3];
                for yy in 0..rh {
                    region[yy * rw * 3..(yy + 1) * rw * 3].copy_from_slice(&rgb[((ry0 + yy) * w + rx0) * 3..((ry0 + yy) * w + rx0 + rw) * 3]);
                }
                let out = run_tile(model, &region, rh, rw, use_avx2);
                // 余白を除いた中心部だけを切り出す。
                let (cw, ch) = (x1 - tx, y1 - ty);
                let mut core = vec![0f32; cw * r * ch * r * 3];
                for yy in 0..ch * r {
                    let src_y = (ty - ry0) * r + yy;
                    let src = &out[(src_y * rw * r + (tx - rx0) * r) * 3..][..cw * r * 3];
                    core[yy * cw * r * 3..(yy + 1) * cw * r * 3].copy_from_slice(src);
                }
                if let Ok(mut r) = results.lock() {
                    r.push((tx, ty, cw, ch, core));
                }
            });
        }
    });
    let mut full = vec![0f32; w * r * h * r * 3];
    for (tx, ty, cw, ch, core) in results.into_inner().unwrap_or_default() {
        for yy in 0..ch * r {
            let dst = ((ty * r + yy) * w * r + tx * r) * 3;
            full[dst..dst + cw * r * 3].copy_from_slice(&core[yy * cw * r * 3..(yy + 1) * cw * r * 3]);
        }
    }
    full
}



