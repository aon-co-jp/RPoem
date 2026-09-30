# open-runo-voice — 声の後処理(共有クレート)

TTS(音声合成)が出したモノラル音声を、キャラクターらしい声に加工する、純Rust・依存クレート無しの共有クレート。
`crates/open-runo-voice`。`wasm32-unknown-unknown`向けのコンパイルは確認済み(ブラウザでの実行は未検証)。

## 何ができるか

| モジュール | 内容 |
|---|---|
| `dsp` | **音程と声の太さ(フォルマント=声道の大きさ)を独立に制御**する変換(`Mode::FormantIndependent`、既定)。長3度違いの2人でハモる合成。前後の無音トリム。音量(RMS)の統一とソフトリミッター。EQ(ローシェルフ/ハイシェルフ/ハイパス)。声のレシピ(`VoiceStyle::Maid`=メイド風、`DeepMale`=太く低い男性) |
| `formant` | FFT+ケプストラムのスペクトル包絡を周波数軸で伸縮する補正(音程=ハーモニクスの位置は変えない)。補正ゲイン上限±24dB |
| `resampler` | Kaiser窓(β=5)の多相リサンプラ。make-diskの`resample_poly`(scipy互換で精度検証済み)と同じ設計。折り返し雑音を出さない |
| `wav` | 16bit PCM WAVの読み書き(ステレオ→モノラル、ストリーミング出力のサイズ0にも対応)。TPDFディザ(`java.util.Random`と同じ乱数系列) |
| `fft` | 小さな基数2のFFT |

## 使い方

```rust
use open_runo_voice::{parse_wav, render, wav_bytes, SourceGender, VoiceStyle};

// TTSエンジンが出したWAV(16bit PCM)を読み、メイド風の声に加工して、ハモらせる
let pcm = parse_wav(&tts_wav_bytes).ok_or("WAVを読めません")?;
let out = render(&pcm, VoiceStyle::Maid, SourceGender::Female, /* harmony */ true, /* pitch_mul */ 1.0);
let wav = wav_bytes(&out); // 音量統一済み、ピーク約0.9
```

`SourceGender`は、TTSエンジンが使った声の性別(音声名からの推定でよい。不明は`Unknown`=女性扱い)。声の太さの目標は
音声学の目安(成人の男性と女性のフォルマントの比はおよそ1.15〜1.2)に基づく: メイド風は女性の声のもとから1.06倍、
太い男性は0.82倍。セリフごとの抑揚は`pitch_mul`(音程だけに掛かり、声の太さは変わらない)。

## 検証(2026-09-30、すべて数値・テストでの検証)

- **旧方式との照合**: 旧Kotlin実装の出力(`tests/golden/*.f32`)と、`Mode::Legacy`の出力を照合。4パターンすべて最大誤差0.00000。
- **音程と声の太さの独立制御**: 直接合成した正解の母音(パルス列を共鳴器3段に通したもの)とのスペクトル包絡の距離。
  音程0.72倍・声の太さ据え置き: 新1.6dB vs 旧10.3dB。音程はそのままで声の太さだけ動かす場合は0.6〜1.1dB(未処理は約8dB)。
  補正ゲインの上限が±12dBだと鋭いフォルマントを動かしきれず、±24dBで解決した(テストで発見)。
- **速度**(4秒の音声、ウォーム最速): 単独34ms、ハモり68ms。

**測っているのはスペクトル包絡の近さで、聴感品質そのものではない。** 声質(自然さ)の評価は、実際に聴いて行うこと。
また、この後処理は元のTTS音声の質を超えない(声の根本の質は、TTSエンジンの音声で決まる)。

## 使っているプロジェクト

- `aon-co-jp/maid-cafe-se`(Windows版・Android版の声)。`maid-cafe-core`が`open_runo_voice`を再公開している。
- `aon-co-jp/open-english`: ローカル版・ミックス版のサーバー側TTSで使う構想(未着手、同リポジトリのCLAUDE.md参照)。
  WEB版はブラウザのWeb Speech APIが出力を取り出せず、後処理できない。

## 関連(このクレートには含まれない)

- AI帯域拡張(LavaSR、tract+ONNX): `maid-cafe-se`の`crates/maid-cafe-enhance`(モデル約56MBの取得・検証、tractの重い依存を含む)。
  RPoem本体のロックファイルへ重い依存を持ち込まないため、当面こちらには入れていない。
