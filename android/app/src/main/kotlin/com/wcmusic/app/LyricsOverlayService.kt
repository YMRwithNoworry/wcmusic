package com.wcmusic.app

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.IBinder
import android.provider.Settings
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.app.NotificationCompat

class LyricsOverlayService : Service() {
    companion object {
        const val ACTION_SHOW = "com.wcmusic.app.lyrics.SHOW"
        const val ACTION_UPDATE = "com.wcmusic.app.lyrics.UPDATE"
        const val ACTION_HIDE = "com.wcmusic.app.lyrics.HIDE"
        const val EXTRA_TITLE = "title"
        const val EXTRA_CURRENT_LINE = "currentLine"
        const val EXTRA_NEXT_LINE = "nextLine"

        private const val CHANNEL_ID = "wcmusic_lyrics"
        private const val NOTIFICATION_ID = 3012
    }

    private lateinit var windowManager: WindowManager
    private var overlayView: View? = null
    private var currentText: TextView? = null
    private var nextText: TextView? = null
    private var layoutParams: WindowManager.LayoutParams? = null

    override fun onCreate() {
        super.onCreate()
        windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        createNotificationChannel()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_HIDE -> {
                removeOverlay()
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_SHOW -> {
                startForeground(NOTIFICATION_ID, createNotification())
                showOverlay()
            }
            ACTION_UPDATE -> {
                startForeground(NOTIFICATION_ID, createNotification())
                showOverlay()
                val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
                val current = intent.getStringExtra(EXTRA_CURRENT_LINE).orEmpty()
                currentText?.text = current.ifBlank { title }
                nextText?.text = intent.getStringExtra(EXTRA_NEXT_LINE).orEmpty()
            }
            else -> return START_NOT_STICKY
        }
        return START_STICKY
    }

    override fun onDestroy() {
        removeOverlay()
        super.onDestroy()
    }

    @SuppressLint("ClickableViewAccessibility")
    private fun showOverlay() {
        if (overlayView != null) return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M && !Settings.canDrawOverlays(this)) {
            stopSelf()
            return
        }
        val density = resources.displayMetrics.density
        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding((20 * density).toInt(), (12 * density).toInt(), (20 * density).toInt(), (12 * density).toInt())
            background = GradientDrawable().apply {
                setColor(Color.argb(222, 28, 31, 27))
                cornerRadius = 12 * density
            }
        }
        currentText = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(Color.rgb(245, 243, 236))
            textSize = 19f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            maxLines = 1
            text = "播放歌曲后将在这里显示歌词"
        }
        nextText = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(Color.rgb(167, 197, 143))
            textSize = 14f
            maxLines = 1
        }
        container.addView(currentText)
        container.addView(nextText)

        val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            @Suppress("DEPRECATION")
            WindowManager.LayoutParams.TYPE_PHONE
        }
        val params = WindowManager.LayoutParams(
            (resources.displayMetrics.widthPixels * 0.9).toInt(),
            WindowManager.LayoutParams.WRAP_CONTENT,
            type,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            android.graphics.PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = (resources.displayMetrics.widthPixels * 0.05).toInt()
            y = (resources.displayMetrics.heightPixels * 0.72).toInt()
        }
        var initialX = 0
        var initialY = 0
        var touchX = 0f
        var touchY = 0f
        container.setOnTouchListener { _, event ->
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    initialX = params.x
                    initialY = params.y
                    touchX = event.rawX
                    touchY = event.rawY
                    true
                }
                MotionEvent.ACTION_MOVE -> {
                    params.x = initialX + (event.rawX - touchX).toInt()
                    params.y = initialY + (event.rawY - touchY).toInt()
                    windowManager.updateViewLayout(container, params)
                    true
                }
                else -> false
            }
        }
        layoutParams = params
        overlayView = container
        windowManager.addView(container, params)
    }

    private fun removeOverlay() {
        overlayView?.let { windowManager.removeView(it) }
        overlayView = null
        currentText = null
        nextText = null
        layoutParams = null
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                "悬浮窗歌词",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "保持 WCMusic 悬浮歌词运行"
                setShowBadge(false)
            },
        )
    }

    private fun createNotification(): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentTitle("WCMusic 悬浮歌词")
            .setContentText("歌词正在随音乐同步")
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setSilent(true)
            .build()
    }
}
