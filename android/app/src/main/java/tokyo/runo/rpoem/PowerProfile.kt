package tokyo.runo.rpoem

import android.content.Context

/**
 * 3電源プロファイル(open-web-server版`PowerProfile.kt`と同じ設計、
 * 2026-07-24新設)。
 *
 * - [POWER_SAVE] 省電力版: 死活監視ポーリング間隔を延ばし、`WakeLock`を
 *   一切取得しない(Android Doze/App Standbyに逆らわない)。
 * - [NORMAL] 通常版: 上記2つの中間。バランス型(既定値)。
 * - [ALWAYS_ON] 常時電源接続版: 充電器に繋ぎっぱなしの端末向け。
 *   `PARTIAL_WAKE_LOCK`を保持し、CPU+GPU+NPUを備えたRPoemインスタンス
 *   向けのハードウェアアクセラレーター対応表示を優先する短いポーリング
 *   間隔にする。
 *
 * **正直な開示**: RPoem自体はこのAndroid端末上で動くサーバープロセス
 * ではなく、どこかで稼働中のインスタンスへ接続するクライアントである
 * ため、ここでの「省電力/常時電源接続」の実体は(1)死活監視ポーリング
 * 間隔の調整、(2)常時電源接続版のみ`WakeLock`取得、の2点のみ
 * (open-web-server版のようにサーバープロセス自体の起動有無を切り替える
 * ものではない)。
 */
enum class PowerProfile(val prefValue: String, val label: String, val emoji: String) {
    POWER_SAVE("power_save", "省電力", "🔋⚡️✕"),
    NORMAL("normal", "通常", "⚖️"),
    ALWAYS_ON("always_on", "常時電源接続", "🔌");

    companion object {
        private const val PREFS_NAME = "rpoem_prefs"
        private const val KEY_PROFILE = "power_profile"

        fun fromPrefValue(value: String?): PowerProfile =
            values().firstOrNull { it.prefValue == value } ?: NORMAL

        fun load(context: Context): PowerProfile {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            return fromPrefValue(prefs.getString(KEY_PROFILE, null))
        }

        fun save(context: Context, profile: PowerProfile) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit().putString(KEY_PROFILE, profile.prefValue).apply()
        }
    }
}
