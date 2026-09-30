//! 声の後処理(aon-co-jpの各プロジェクトで共有する、純Rust・依存無し)。
//!
//! TTS(音声合成)が出したモノラル音声を、キャラクターらしい声に加工する:
//!
//! - [`dsp`] — 音程と声の太さ(フォルマント=声道の大きさ)を**独立に**制御する変換([`Mode::FormantIndependent`]、既定)、
//!   長3度違いの2人でハモる合成、無音トリム、音量(RMS)の統一とソフトリミッター、EQ(シェルフ)
//! - [`formant`] — FFT+ケプストラムのスペクトル包絡を周波数軸で伸縮する補正(音程=ハーモニクスの位置は変えない)
//! - [`resampler`] — Kaiser窓の多相リサンプラ(make-diskの`resample_poly`、scipy互換で精度検証済みの設計)
//! - [`wav`] — 16bit PCM WAVの読み書き(ステレオ→モノラル、ストリーミング出力のサイズ0にも対応)、TPDFディザ
//! - [`fft`] — 小さな基数2のFFT
//!
//! 由来と検証: `aon-co-jp/maid-cafe-se`(Kotlin実装→Rust移植)。旧Kotlin版の出力とは数値照合済み
//! (`tests/golden.rs`、[`Mode::Legacy`])。新方式は、直接合成した正解の母音との包絡距離で検証
//! (`tests/formant.rs`: 新1.6dB vs 旧10.3dB)。**測っているのはスペクトル包絡の近さで、聴感品質そのものではない。**
//!
//! 使っているプロジェクト: maid-cafe-se(Windows/Android)。open-english(ローカル版・ミックス版のサーバー側TTS)は
//! これから。`wasm32-unknown-unknown`向けのコンパイルは確認済み(ブラウザでの実行は未検証)。

pub mod dsp;
pub mod fft;
pub mod formant;
pub mod resampler;
pub mod wav;

pub use dsp::{recipe, render, render_with, render_with_recipe, Mode, Recipe, SourceGender};
pub use wav::{parse_wav, to_pcm16, wav_bytes, Pcm};

/// 声のキャラクター。`Maid`=メイドカフェ風(高め・明るい)、`DeepMale`=太くて低い男性。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoiceStyle {
    Maid,
    DeepMale,
}

impl VoiceStyle {
    pub const ALL: [VoiceStyle; 2] = [VoiceStyle::Maid, VoiceStyle::DeepMale];

    /// 保存形式での名前(maid-cafe-seのAndroid/Kotlin版と同じ)。
    pub fn name(self) -> &'static str {
        match self {
            VoiceStyle::Maid => "MAID",
            VoiceStyle::DeepMale => "DEEP_MALE",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == s)
    }

    /// 画面表示用の名前。
    pub fn display(self) -> &'static str {
        match self {
            VoiceStyle::Maid => "メイドカフェ風",
            VoiceStyle::DeepMale => "太く低い男性",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_style_names_round_trip() {
        for v in VoiceStyle::ALL {
            assert_eq!(Some(v), VoiceStyle::from_name(v.name()));
            assert!(!v.display().is_empty());
        }
        assert_eq!(None, VoiceStyle::from_name("nobody"));
    }
}
