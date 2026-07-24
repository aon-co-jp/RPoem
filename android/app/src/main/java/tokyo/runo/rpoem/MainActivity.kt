package tokyo.runo.rpoem

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.SharedPreferences
import android.os.Bundle
import android.os.PowerManager
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * RPoem(open-runo-router、Tomcat相当の汎用アプリケーションサーバー層)
 * 向けAndroidクライアント。
 *
 * **設計方針(open-web-server版との違い、正直な開示)**: open-web-server
 * 版は本体をクロスコンパイルしたネイティブ実行ファイルをAPKへ同梱し
 * `ProcessBuilder`で起動する構成だったが、RPoem(open-runo-router)は
 * 一般にVPS/デスクトップ/LAN内サーバー上で常時稼働させる汎用アプリ
 * ケーションサーバーであり、この端末上でプロセスとして起動する使い方は
 * 現実的でない(過剰なフル機能移植を避ける方針にも合わない)。そのため
 * 本アプリは「どこかで稼働中のRPoemインスタンスへの接続設定+死活監視
 * クライアント」として実装する——接続先(ホスト:ポート)を入力し、
 * `GET /health`を叩いて実際に応答することを画面上で確認できるように
 * する。
 *
 * 電源プロファイル(省電力/通常/常時電源接続)自体は、open-web-server版と
 * 同じ3モード構成・同じ電源切断/再接続ダイアログ導線を持つ。実体は
 * (1) 死活監視ポーリング間隔の調整、(2) 常時電源接続版のみ`WakeLock`
 * 取得、の2点(サーバープロセスの起動有無は切り替えない、上記の理由)。
 */
class MainActivity : AppCompatActivity() {

    companion object {
        const val EXTRA_PROFILE = "profile"
        private const val PREFS_NAME = "rpoem_prefs"
        private const val KEY_SERVER_URL = "server_url"
        private const val DEFAULT_SERVER_URL = "http://127.0.0.1:8080"
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private var healthPollJob: Job? = null
    private var powerConnectionReceiver: BroadcastReceiver? = null
    private lateinit var currentProfile: PowerProfile
    private lateinit var prefs: SharedPreferences

    /**
     * プロファイルごとの死活監視ポーリング間隔(open-web-server版
     * `healthPollIntervalMs`と同じ考え方: 省電力は間隔を大きく延ばし
     * Doze/App Standbyへの影響を最小化、常時電源接続は短い間隔で
     * 即応性を優先する)。
     */
    private fun healthPollIntervalMs(profile: PowerProfile): Long = when (profile) {
        PowerProfile.POWER_SAVE -> 5 * 60_000L // 5分
        PowerProfile.NORMAL -> 60_000L // 1分
        PowerProfile.ALWAYS_ON -> 5_000L // 5秒
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        prefs = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        currentProfile = resolveProfile()
        PowerProfile.save(this, currentProfile)

        val statusText = findViewById<TextView>(R.id.statusText)
        val logText = findViewById<TextView>(R.id.logText)
        val serverUrlInput = findViewById<EditText>(R.id.serverUrlInput)
        val connectButton = findViewById<Button>(R.id.connectButton)
        val changeProfileButton = findViewById<Button>(R.id.changeProfileButton)

        serverUrlInput.setText(prefs.getString(KEY_SERVER_URL, DEFAULT_SERVER_URL))
        statusText.text = "RPoem [${currentProfile.emoji} ${currentProfile.label}モード] (未接続)"

        applyProfilePowerBehavior(logText)

        connectButton.setOnClickListener {
            val serverUrl = serverUrlInput.text.toString().trim().trimEnd('/')
            if (serverUrl.isEmpty()) {
                Toast.makeText(this, "接続先URLを入力してください", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            prefs.edit().putString(KEY_SERVER_URL, serverUrl).apply()
            connectButton.isEnabled = false
            CoroutineScope(Dispatchers.Main).launch {
                statusText.text = "[${currentProfile.emoji} ${currentProfile.label}] 接続確認中..."
                val log = StringBuilder()
                log.appendLine("target: $serverUrl/health")
                val ok = withContext(Dispatchers.IO) { checkHealth(serverUrl, log) }
                statusText.text = if (ok) {
                    "[${currentProfile.emoji} ${currentProfile.label}] 接続OK — GET /health が200を返しました"
                } else {
                    "[${currentProfile.emoji} ${currentProfile.label}] 接続失敗(下記ログ参照)"
                }
                logText.text = log.toString()
                connectButton.isEnabled = true
                if (ok) {
                    startPeriodicHealthPoll(serverUrl, statusText)
                }
            }
        }

        changeProfileButton.setOnClickListener {
            startActivity(Intent(this, ProfileSelectActivity::class.java))
            finish()
        }

        registerPowerConnectionReceiver()
    }

    /**
     * 電源の抜き差しを監視する(open-web-server版と同じ導線)。
     * - 常時電源接続版の実行中に電源が外れたら「省電力モードに
     *   切り替えますか?それとも通常モードのままにしますか?」と質問
     *   (既定推奨=省電力)。
     * - 省電力/通常版の実行中に電源が再接続されたら常時電源接続版へ
     *   戻すかを尋ねる。
     */
    private fun registerPowerConnectionReceiver() {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                when (intent.action) {
                    Intent.ACTION_POWER_DISCONNECTED -> onPowerDisconnected()
                    Intent.ACTION_POWER_CONNECTED -> onPowerConnected()
                }
            }
        }
        powerConnectionReceiver = receiver
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_POWER_DISCONNECTED)
            addAction(Intent.ACTION_POWER_CONNECTED)
        }
        registerReceiver(receiver, filter)
    }

    private fun onPowerDisconnected() {
        if (currentProfile != PowerProfile.ALWAYS_ON) return
        if (isFinishing || isDestroyed) return
        AlertDialog.Builder(this)
            .setTitle("電源が外れました")
            .setMessage(
                "常時電源接続モードで動作中に電源が外れました。\n" +
                    "省電力モードに切り替えますか?それとも通常モードの" +
                    "ままにしますか?\n(推奨: 省電力モード)"
            )
            .setPositiveButton("省電力モードへ切替") { _, _ ->
                switchProfileAndRestart(PowerProfile.POWER_SAVE)
            }
            .setNegativeButton("通常モードのままにする") { _, _ ->
                switchProfileAndRestart(PowerProfile.NORMAL)
            }
            .setCancelable(false)
            .show()
    }

    private fun onPowerConnected() {
        if (currentProfile == PowerProfile.ALWAYS_ON) return
        if (isFinishing || isDestroyed) return
        AlertDialog.Builder(this)
            .setTitle("電源が接続されました")
            .setMessage("常時電源接続モード(ハードウェアアクセラレーター対応表示を優先)に切り替えますか?")
            .setPositiveButton("常時電源接続へ切替") { _, _ ->
                switchProfileAndRestart(PowerProfile.ALWAYS_ON)
            }
            .setNegativeButton("このままにする", null)
            .show()
    }

    private fun switchProfileAndRestart(newProfile: PowerProfile) {
        PowerProfile.save(this, newProfile)
        Toast.makeText(
            this,
            "${newProfile.emoji} ${newProfile.label}モードへ切り替えます",
            Toast.LENGTH_SHORT
        ).show()
        val intent = Intent(this, MainActivity::class.java)
        intent.putExtra(EXTRA_PROFILE, newProfile.prefValue)
        startActivity(intent)
        finish()
    }

    private fun resolveProfile(): PowerProfile {
        return when (intent?.action) {
            "tokyo.runo.rpoem.LAUNCH_POWER_SAVE" -> PowerProfile.POWER_SAVE
            "tokyo.runo.rpoem.LAUNCH_NORMAL" -> PowerProfile.NORMAL
            "tokyo.runo.rpoem.LAUNCH_ALWAYS_ON" -> PowerProfile.ALWAYS_ON
            else -> {
                val extra = intent?.getStringExtra(EXTRA_PROFILE)
                if (extra != null) PowerProfile.fromPrefValue(extra) else PowerProfile.load(this)
            }
        }
    }

    /**
     * 省電力/通常は`WakeLock`を一切取得しない(Doze/App Standbyに逆らわ
     * ない、これが「省電力対応」の実体)。常時電源接続のみ
     * `PARTIAL_WAKE_LOCK`を保持する。
     */
    private fun applyProfilePowerBehavior(logText: TextView) {
        val note = when (currentProfile) {
            PowerProfile.ALWAYS_ON -> {
                try {
                    val pm = getSystemService(POWER_SERVICE) as PowerManager
                    val lock = pm.newWakeLock(
                        PowerManager.PARTIAL_WAKE_LOCK,
                        "RPoem::AlwaysOnWakeLock"
                    )
                    lock.acquire()
                    wakeLock = lock
                    "power: acquired PARTIAL_WAKE_LOCK (always-on profile)"
                } catch (e: Exception) {
                    "power: failed to acquire WakeLock: ${e.message}"
                }
            }
            PowerProfile.POWER_SAVE ->
                "power: no WakeLock acquired (power-save profile, Doze-friendly, poll every 5min)"
            PowerProfile.NORMAL ->
                "power: no WakeLock acquired (normal profile, poll every 1min)"
        }
        logText.text = note
    }

    private fun checkHealth(serverUrl: String, log: StringBuilder): Boolean {
        return try {
            val url = URL("$serverUrl/health")
            val conn = url.openConnection() as HttpURLConnection
            conn.connectTimeout = 3000
            conn.readTimeout = 3000
            val code = conn.responseCode
            val body = conn.inputStream.bufferedReader().readText()
            conn.disconnect()
            log.appendLine("GET /health -> $code \"$body\"")
            code == 200
        } catch (e: Exception) {
            log.appendLine("GET /health failed: ${e.message}")
            false
        }
    }

    /**
     * 接続確認後の継続的な死活監視。プロファイルごとに間隔を変える
     * (`healthPollIntervalMs`)ことが「省電力版が実際に省電力になる」
     * 施策そのもの——省電力版はこのループの頻度自体を大きく落とし、
     * CPU/無線を起こす回数を最小化する。
     */
    private fun startPeriodicHealthPoll(serverUrl: String, statusText: TextView) {
        healthPollJob?.cancel()
        val intervalMs = healthPollIntervalMs(currentProfile)
        healthPollJob = CoroutineScope(Dispatchers.Main).launch {
            while (isActive) {
                delay(intervalMs)
                val ok = withContext(Dispatchers.IO) {
                    try {
                        val url = URL("$serverUrl/health")
                        val conn = url.openConnection() as HttpURLConnection
                        conn.connectTimeout = 3000
                        conn.readTimeout = 3000
                        val code = conn.responseCode
                        conn.disconnect()
                        code == 200
                    } catch (_: Exception) {
                        false
                    }
                }
                statusText.text = if (ok) {
                    "[${currentProfile.emoji} ${currentProfile.label}] RUNNING " +
                        "(poll every ${intervalMs / 1000}s)"
                } else {
                    "[${currentProfile.emoji} ${currentProfile.label}] health check failed"
                }
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        healthPollJob?.cancel()
        powerConnectionReceiver?.let {
            try {
                unregisterReceiver(it)
            } catch (_: IllegalArgumentException) {
                // 未登録のまま呼ばれても無視する。
            }
        }
        if (wakeLock?.isHeld == true) {
            wakeLock?.release()
        }
    }
}
