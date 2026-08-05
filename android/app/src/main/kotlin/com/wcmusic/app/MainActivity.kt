package com.wcmusic.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.ContextCompat
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

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
                        )
                        result.success(null)
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

    private fun updateLyrics(title: String, currentLine: String, nextLine: String) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && !Settings.canDrawOverlays(this)) return
        ContextCompat.startForegroundService(this, Intent(this, LyricsOverlayService::class.java).apply {
            action = LyricsOverlayService.ACTION_UPDATE
            putExtra(LyricsOverlayService.EXTRA_TITLE, title)
            putExtra(LyricsOverlayService.EXTRA_CURRENT_LINE, currentLine)
            putExtra(LyricsOverlayService.EXTRA_NEXT_LINE, nextLine)
        })
    }
}
