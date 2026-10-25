package com.wcmusic.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    private val channelName = "wcmusic/lyrics_overlay"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, channelName)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "setEnabled" -> {
                        val enabled = call.argument<Boolean>("enabled") == true
                        result.success(setLyricsEnabled(enabled))
                    }
                    "update" -> {
                        updateLyrics(
                            call.argument<String>("title").orEmpty(),
                            call.argument<String>("currentLine").orEmpty(),
                            call.argument<String>("nextLine").orEmpty(),
                            call.argument<String>("currentTranslation").orEmpty(),
                            call.argument<String>("nextTranslation").orEmpty(),
                        )
                        result.success(null)
                    }
                    "installApk" -> {
                        result.success(installApk(call.argument<String>("path").orEmpty()))
                    }
                    else -> result.notImplemented()
                }
            }
    }

    private fun setLyricsEnabled(enabled: Boolean): Boolean {
        if (!enabled) {
            stopService(Intent(this, LyricsOverlayService::class.java))
            return true
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && !Settings.canDrawOverlays(this)) {
            startActivity(
                Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    Uri.parse("package:$packageName"),
                ),
            )
            return false
        }
        val intent = Intent(this, LyricsOverlayService::class.java).apply {
            action = LyricsOverlayService.ACTION_SHOW
        }
        ContextCompat.startForegroundService(this, intent)
        return true
    }

    private fun updateLyrics(
        title: String,
        currentLine: String,
        nextLine: String,
        currentTranslation: String,
        nextTranslation: String,
    ) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && !Settings.canDrawOverlays(this)) return
        ContextCompat.startForegroundService(this, Intent(this, LyricsOverlayService::class.java).apply {
            action = LyricsOverlayService.ACTION_UPDATE
            putExtra(LyricsOverlayService.EXTRA_TITLE, title)
            putExtra(LyricsOverlayService.EXTRA_CURRENT_LINE, currentLine)
            putExtra(LyricsOverlayService.EXTRA_NEXT_LINE, nextLine)
            putExtra(LyricsOverlayService.EXTRA_CURRENT_TRANSLATION, currentTranslation)
            putExtra(LyricsOverlayService.EXTRA_NEXT_TRANSLATION, nextTranslation)
        })
    }

    /** 用系统安装器安装下载好的 APK；Android 8+ 需要「安装未知应用」权限。 */
    private fun installApk(path: String): Boolean {
        val file = File(path)
        if (!file.exists()) return false
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !packageManager.canRequestPackageInstalls()
        ) {
            startActivity(
                Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:$packageName"),
                ),
            )
            return false
        }
        val authority = "$packageName.fileprovider"
        val uri = FileProvider.getUriForFile(this, authority, file)
        startActivity(
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            },
        )
        return true
    }
}
