// TALLY's Android glue, used in place of dx's generated MainActivity.kt
// (Dioxus.toml: application.android_main_activity). The activity itself
// is dx's stock one; the rest is the platform side of habit reminders —
// alarms, notifications and their actions. Rust decides *whether* and
// *when* (src/reminders.rs), calling in through TallyReminders and back
// out through the receivers' native methods (src/android.rs).

package dev.dioxus.main

import android.Manifest
import android.app.Activity
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import java.util.Calendar

typealias BuildConfig = com.huuff.tally.BuildConfig

class MainActivity : WryActivity()

object TallyReminders {
    private const val CHANNEL = "reminders"
    private const val SNOOZE_MS = 60 * 60 * 1000L
    private const val PERMISSION_REQUEST = 7001
    const val ACTION_FIRE = "com.huuff.tally.REMINDER_FIRE"
    const val ACTION_SNOOZED = "com.huuff.tally.REMINDER_SNOOZED"
    const val ACTION_DONE = "com.huuff.tally.REMINDER_DONE"
    const val ACTION_LATER = "com.huuff.tally.REMINDER_LATER"
    const val EXTRA_ID = "habit_id"

    // PendingIntents match on action + request code, so each (action,
    // habit) pair is its own alarm or button.
    private fun broadcast(ctx: Context, action: String, id: Long): PendingIntent =
        PendingIntent.getBroadcast(
            ctx,
            id.toInt(),
            Intent(ctx, ReminderReceiver::class.java).setAction(action).putExtra(EXTRA_ID, id),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun alarm(ctx: Context, at: Long, intent: PendingIntent) {
        val alarms = ctx.getSystemService(AlarmManager::class.java)
        if (Build.VERSION.SDK_INT < 31 || alarms.canScheduleExactAlarms()) {
            alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, intent)
        } else {
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, intent)
        }
    }

    /** Arm (or move) the habit's reminder, at a local wall-clock time. */
    @JvmStatic
    fun schedule(ctx: Context, id: Long, year: Int, month: Int, day: Int, hour: Int, minute: Int) {
        val at = Calendar.getInstance().apply {
            clear()
            set(year, month - 1, day, hour, minute)
        }.timeInMillis
        alarm(ctx, at, broadcast(ctx, ACTION_FIRE, id))
    }

    /** Disarm the habit's reminder. A pending "Later" re-checks on its own. */
    @JvmStatic
    fun cancel(ctx: Context, id: Long) {
        ctx.getSystemService(AlarmManager::class.java).cancel(broadcast(ctx, ACTION_FIRE, id))
    }

    fun snooze(ctx: Context, id: Long) {
        dismiss(ctx, id)
        alarm(ctx, System.currentTimeMillis() + SNOOZE_MS, broadcast(ctx, ACTION_SNOOZED, id))
    }

    @JvmStatic
    fun dismiss(ctx: Context, id: Long) {
        NotificationManagerCompat.from(ctx).cancel(id.toInt())
    }

    private fun builder(ctx: Context, channelName: String, title: String, text: String) =
        NotificationCompat.Builder(ctx, ensureChannel(ctx, channelName))
            .setSmallIcon(smallIcon(ctx))
            .setContentTitle(title)
            .setContentText(text)
            .setCategory(NotificationCompat.CATEGORY_REMINDER)
            .setAutoCancel(true)
            .setContentIntent(openApp(ctx))

    /** The reminder itself; `later` is null on the snoozed re-fire. */
    @JvmStatic
    fun notify(
        ctx: Context,
        id: Long,
        channelName: String,
        title: String,
        text: String,
        done: String,
        later: String?,
    ) {
        if (!allowed(ctx)) return
        val notification = builder(ctx, channelName, title, text)
            .addAction(0, done, broadcast(ctx, ACTION_DONE, id))
        if (later != null) notification.addAction(0, later, broadcast(ctx, ACTION_LATER, id))
        post(ctx, id, notification)
    }

    /** Replaces the reminder once Done has checked the habit in. */
    @JvmStatic
    fun confirmDone(ctx: Context, id: Long, channelName: String, title: String, text: String) {
        if (!allowed(ctx)) return
        post(ctx, id, builder(ctx, channelName, title, text).setOnlyAlertOnce(true).setSilent(true))
    }

    @Suppress("MissingPermission") // allowed() checked by every caller
    private fun post(ctx: Context, id: Long, notification: NotificationCompat.Builder) {
        try {
            NotificationManagerCompat.from(ctx).notify(id.toInt(), notification.build())
        } catch (_: SecurityException) {
            // Permission revoked between the check and the post.
        }
    }

    /** Notifications permitted, and the reminder channel not switched off. */
    @JvmStatic
    fun allowed(ctx: Context): Boolean {
        if (!NotificationManagerCompat.from(ctx).areNotificationsEnabled()) return false
        if (Build.VERSION.SDK_INT < 26) return true
        val channel = ctx.getSystemService(NotificationManager::class.java).getNotificationChannel(CHANNEL)
        return channel == null || channel.importance != NotificationManager.IMPORTANCE_NONE
    }

    @JvmStatic
    fun requestPermission(ctx: Context) {
        if (Build.VERSION.SDK_INT < 33) return
        val activity = ctx as? Activity ?: return
        activity.runOnUiThread {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                PERMISSION_REQUEST,
            )
        }
    }

    @JvmStatic
    fun openSettings(ctx: Context) {
        val intent = if (Build.VERSION.SDK_INT >= 26) {
            Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                .putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName)
        } else {
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", ctx.packageName, null))
        }
        ctx.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    private fun ensureChannel(ctx: Context, name: String): String {
        if (Build.VERSION.SDK_INT >= 26) {
            // Re-creating updates the name (language changes) and keeps
            // whatever the user set for the channel.
            ctx.getSystemService(NotificationManager::class.java).createNotificationChannel(
                NotificationChannel(CHANNEL, name, NotificationManager.IMPORTANCE_DEFAULT),
            )
        }
        return CHANNEL
    }

    // Looked up by name: dx compiles this file before TALLY's res/ is
    // copied into the generated project (see README), so R.drawable
    // wouldn't resolve on that first pass.
    private fun smallIcon(ctx: Context): Int =
        ctx.resources.getIdentifier("ic_stat_tally", "drawable", ctx.packageName)
            .takeIf { it != 0 } ?: ctx.applicationInfo.icon

    private fun openApp(ctx: Context): PendingIntent? =
        ctx.packageManager.getLaunchIntentForPackage(ctx.packageName)?.let {
            PendingIntent.getActivity(ctx, 0, it, PendingIntent.FLAG_IMMUTABLE)
        }
}

/** Alarms and notification buttons. Not exported: only our own PendingIntents reach it. */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        val id = intent.getLongExtra(TallyReminders.EXTRA_ID, -1)
        if (id < 0) return
        when (intent.action) {
            TallyReminders.ACTION_FIRE -> nativeFire(ctx, id, false)
            TallyReminders.ACTION_SNOOZED -> nativeFire(ctx, id, true)
            TallyReminders.ACTION_DONE -> nativeDone(ctx, id)
            TallyReminders.ACTION_LATER -> TallyReminders.snooze(ctx, id)
        }
    }

    private external fun nativeFire(ctx: Context, id: Long, snoozed: Boolean)
    private external fun nativeDone(ctx: Context, id: Long)

    companion object {
        init {
            System.loadLibrary("main")
        }
    }
}

/** Boot, clock and time-zone changes, app updates: re-arm every reminder. */
class RescheduleReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        // Exported for the system broadcasts; ignore anything else.
        if (intent.action in ACTIONS) nativeSync(ctx)
    }

    private external fun nativeSync(ctx: Context)

    companion object {
        private val ACTIONS = setOf(
            Intent.ACTION_BOOT_COMPLETED,
            Intent.ACTION_MY_PACKAGE_REPLACED,
            Intent.ACTION_TIME_CHANGED,
            Intent.ACTION_TIMEZONE_CHANGED,
        )

        init {
            System.loadLibrary("main")
        }
    }
}
