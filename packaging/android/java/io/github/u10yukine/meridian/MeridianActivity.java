// Adapted from Starling (https://github.com/starling-client/starling, 5432370d5736,
// `packaging/android/java/io/github/juanluci/starling/StarlingActivity.java`), written by Vasysik
// and the Starling contributors. Copyright (c) 2026 Ludovic Marie and the Horizon contributors,
// under the same MIT license as Horizon (LICENSE at the repository root).
// SPDX-License-Identifier: MIT
package io.github.u10yukine.meridian;

import android.app.ActivityManager;
import android.app.ApplicationExitInfo;
import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.net.Uri;
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Process;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;
import android.provider.MediaStore;
import android.provider.OpenableColumns;
import android.provider.Settings;
import android.util.Log;
import android.view.DisplayCutout;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

/**
 * The game runs in libmeridian.so (NativeActivity); this only does what the NDK cannot: hides the
 * system bars, and captures the mouse. The game calls the public methods from its own thread
 * (meridian-android's activity.rs). Adapted from Starling's StarlingActivity
 * (packaging/android/java) at 5432370d5736.
 */
public class MeridianActivity extends NativeActivity {
    /** The game holds the mouse (capturePointer); on the UI thread only. */
    private boolean pointerWanted;

    /** How far the on-screen keyboard covers the window, in pixels; 0 when it's away. */
    private volatile int imeHeight;
    private volatile int[] safeInsets = new int[4];

    private static final int PICK_SKIN = 1;
    private static final int PICK_IMPORT = 2;
    /** The largest skin the launcher takes (meridian-launcher's skins::MAX_PNG); one byte more
     *  is copied, so a bigger file reads as too big rather than as a cut-off PNG. */
    private static final int MAX_SKIN = 1024 * 1024;
    private volatile String pendingImportDest;
    private volatile long pendingImportMaxBytes;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        hideSystemBars();
        // ponytail: Android 8 to 10 don't report the keyboard's height this way (0 there); read
        // getWindowVisibleDisplayFrame if players on them need the fields lifted.
        if (Build.VERSION.SDK_INT >= 28) {
            getWindow().getDecorView().setOnApplyWindowInsetsListener((view, insets) -> {
                if (Build.VERSION.SDK_INT >= 30) {
                    imeHeight = insets.getInsets(WindowInsets.Type.ime()).bottom;
                }
                DisplayCutout cutout = insets.getDisplayCutout();
                safeInsets = cutout == null ? new int[4] : new int[] {
                    cutout.getSafeInsetLeft(), cutout.getSafeInsetTop(),
                    cutout.getSafeInsetRight(), cutout.getSafeInsetBottom(),
                };
                return view.onApplyWindowInsets(insets);
            });
        }
    }

    /** The display cutout's (a notch's or a camera hole's) insets in window pixels, as left, top,
     *  right, bottom: the launcher keeps its controls clear of them. Zero without a cutout. */
    public int[] safeInsets() {
        return safeInsets;
    }

    /** The keyboard's height in window pixels (0 when it's away), for the game to lift its text
     *  fields above it: the fullscreen window doesn't shrink for it. */
    public int imeHeight() {
        return imeHeight;
    }

    /**
     * The status and navigation bars hide; a swipe from the edge shows them for a moment, so a
     * stray swipe or press can't leave the game (immersive mode).
     */
    @SuppressWarnings("deprecation")
    private void hideSystemBars() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = getWindow().getInsetsController();
            if (controller != null) {
                controller.hide(WindowInsets.Type.systemBars());
                controller.setSystemBarsBehavior(
                        WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }

    /**
     * Capture the mouse for the game's mouse look, or let it go: captured, its pointer hides and
     * its moves come to the game as relative motion, through NativeActivity's input queue like
     * every other event. Fingers are not affected.
     */
    public void capturePointer(final boolean capture) {
        runOnUiThread(() -> {
            pointerWanted = capture;
            if (capture) {
                getWindow().getDecorView().requestPointerCapture();
            } else {
                getWindow().getDecorView().releasePointerCapture();
            }
        });
    }

    /**
     * Start the app again in a fresh process once this one ends (RestartActivity): the launcher
     * and the game each run in their own. The game ends this process right after asking.
     */
    public void restart() {
        startActivity(new Intent(this, RestartActivity.class)
                .putExtra(RestartActivity.AFTER, Process.myPid())
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        runOnUiThread(() -> {
            // Nothing left on screen for Android to bring back when the process ends.
            finish();
            overridePendingTransition(0, 0);
        });
    }

    /** The browser on an https:// page (the Microsoft sign-in); false where none can. */
    public boolean openLink(String url) {
        if (!url.startsWith("https://")) {
            return false;
        }
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException | SecurityException e) {
            return false;
        }
    }

    /** Puts `text` on the clipboard (the launcher's Copy on its game log). */
    public void copyText(String text) {
        runOnUiThread(() -> {
            ClipboardManager clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            if (clipboard != null) {
                clipboard.setPrimaryClip(ClipData.newPlainText("Meridian game log", text));
            }
        });
    }

    /**
     * Why this app's earlier processes ended (Android 11 and newer; empty before), one per line:
     * the OS's own record, for a diagnostic report. Reason, status, importance, time and the
     * system's description only: no traces.
     */
    public String exitReasons() {
        if (Build.VERSION.SDK_INT < 30) {
            return "";
        }
        ActivityManager manager = (ActivityManager) getSystemService(ACTIVITY_SERVICE);
        if (manager == null) {
            return "";
        }
        StringBuilder text = new StringBuilder();
        for (ApplicationExitInfo info : manager.getHistoricalProcessExitReasons(getPackageName(), 0, 10)) {
            text.append("time=").append(info.getTimestamp())
                    .append(" reason=").append(info.getReason())
                    .append(" status=").append(info.getStatus())
                    .append(" importance=").append(info.getImportance())
                    .append(" description=").append(info.getDescription())
                    .append('\n');
        }
        return text.toString();
    }

    /**
     * Offers the diagnostic ZIP at {@code path} to the system's share sheet, through
     * {@link DiagnosticsProvider}. False where the file isn't one it serves.
     */
    public boolean shareFile(final String path) {
        File file = new File(path);
        Uri uri = DiagnosticsProvider.uriFor(this, file);
        if (uri == null) {
            return false;
        }
        Intent send = new Intent(Intent.ACTION_SEND)
                .setType("application/zip")
                .putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        // setClipData returns nothing, so it can't sit in the chain above.
        send.setClipData(ClipData.newRawUri(file.getName(), uri));
        String title = file.getName().startsWith("meridian-world-")
                ? "Share world"
                : "Share diagnostics";
        final Intent chooser = Intent.createChooser(send, title)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        runOnUiThread(() -> {
            try {
                startActivity(chooser);
            } catch (ActivityNotFoundException | SecurityException e) {
                Log.w("Meridian", "No share target opened", e);
            }
        });
        return true;
    }

    /** The system's file picker for a PNG skin; the chosen file lands in files/picked-skin.png,
     *  where the launcher reads it. False where no picker can open. */
    public boolean pickSkin() {
        Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType("image/png");
        if (pick.resolveActivity(getPackageManager()) == null) {
            return false;
        }
        runOnUiThread(() -> {
            try {
                startActivityForResult(pick, PICK_SKIN);
            } catch (ActivityNotFoundException | SecurityException e) {
                Log.w("Meridian", "No file picker opened", e);
            }
        });
        return true;
    }

    /** The system's file picker for an importable file; the chosen file lands in files/<destName>,
     *  its display name in files/<destName>.name, or files/<destName>.cancelled on cancel. */
    public boolean pickImport(String mime, String destName, long maxBytes) {
        if (destName == null || destName.isEmpty() || destName.contains("/")
                || destName.contains("\\") || destName.contains("..") || maxBytes <= 0) {
            return false;
        }
        Intent pick = new Intent(Intent.ACTION_OPEN_DOCUMENT)
                .addCategory(Intent.CATEGORY_OPENABLE)
                .setType(mime);
        if (pick.resolveActivity(getPackageManager()) == null) {
            return false;
        }
        pendingImportDest = destName;
        pendingImportMaxBytes = maxBytes;
        runOnUiThread(() -> {
            try {
                startActivityForResult(pick, PICK_IMPORT);
            } catch (ActivityNotFoundException | SecurityException e) {
                Log.w("Meridian", "No file picker opened", e);
            }
        });
        return true;
    }

    /** Copies a saved PNG screenshot into Android's Pictures/Meridian gallery (API 29+). */
    public boolean saveScreenshot(final String path) {
        if (Build.VERSION.SDK_INT < 29 || path == null || path.isEmpty()) {
            return false;
        }
        final File src = new File(path);
        if (!src.isFile()) {
            return false;
        }
        new Thread(() -> {
            ContentValues values = new ContentValues();
            values.put(MediaStore.Images.Media.DISPLAY_NAME, src.getName());
            values.put(MediaStore.Images.Media.MIME_TYPE, "image/png");
            values.put(MediaStore.Images.Media.RELATIVE_PATH,
                    Environment.DIRECTORY_PICTURES + "/Meridian");
            values.put(MediaStore.Images.Media.IS_PENDING, 1);
            Uri uri = null;
            try {
                uri = getContentResolver().insert(
                        MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values);
                if (uri == null) {
                    return;
                }
                try (InputStream in = new FileInputStream(src);
                     OutputStream out = getContentResolver().openOutputStream(uri)) {
                    if (out == null) {
                        getContentResolver().delete(uri, null, null);
                        return;
                    }
                    byte[] buffer = new byte[64 * 1024];
                    for (int n; (n = in.read(buffer)) > 0; ) {
                        out.write(buffer, 0, n);
                    }
                }
                ContentValues done = new ContentValues();
                done.put(MediaStore.Images.Media.IS_PENDING, 0);
                getContentResolver().update(uri, done, null, null);
            } catch (Exception e) {
                Log.w("Meridian", "Could not save screenshot to gallery", e);
                if (uri != null) {
                    try {
                        getContentResolver().delete(uri, null, null);
                    } catch (Exception ignored) {
                    }
                }
            }
        }, "screenshot-gallery").start();
        return true;
    }

    /**
     * Buzzes the phone through steps of {@code timings} (milliseconds) at {@code amplitudes}
     * (1 to 255), as a game event feels: a block broken, a hit taken. A phone without a motor
     * stays still. Adapted from Starling's vibrate, which plays one step.
     */
    @SuppressWarnings("deprecation")
    public void vibrate(long[] timings, int[] amplitudes) {
        try {
            Vibrator motor = motor();
            Log.d("MeridianHaptics", "vibrate steps=" + timings.length);
            if (motor == null || !motor.hasVibrator() || timings.length == 0) {
                return;
            }
            if (motor.hasAmplitudeControl()) {
                motor.vibrate(VibrationEffect.createWaveform(timings, amplitudes, -1));
            } else {
                // A timings-only waveform alternates off and on: off for nothing, then on for as
                // long as the steps last in all.
                long total = 0;
                for (long millis : timings) {
                    total += millis;
                }
                motor.vibrate(VibrationEffect.createWaveform(new long[] {0, total}, -1));
            }
        } catch (RuntimeException e) {
            Log.w("Meridian", "Could not vibrate", e);
        }
    }

    /** The phone's default vibration motor; null where there is none. */
    @SuppressWarnings("deprecation")
    private Vibrator motor() {
        if (Build.VERSION.SDK_INT >= 31) {
            VibratorManager motors =
                    (VibratorManager) getSystemService(Context.VIBRATOR_MANAGER_SERVICE);
            return motors == null ? null : motors.getDefaultVibrator();
        }
        return (Vibrator) getSystemService(Context.VIBRATOR_SERVICE);
    }

    /** A short tick for a touch control pressed, unless the phone's own touch feedback is off.
     *  The view's VIRTUAL_KEY haptic would follow that setting by itself, but some phones map
     *  no vibration to it (Samsung's: "vibration absent for constant 1"), so the tick is played
     *  directly. */
    public void tick() {
        try {
            if (Settings.System.getInt(getContentResolver(),
                    Settings.System.HAPTIC_FEEDBACK_ENABLED, 1) == 0) {
                return;
            }
            Vibrator motor = motor();
            Log.d("MeridianHaptics", "tick");
            if (motor == null || !motor.hasVibrator()) {
                return;
            }
            motor.vibrate(Build.VERSION.SDK_INT >= 29
                    ? VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
                    : VibrationEffect.createOneShot(15, VibrationEffect.DEFAULT_AMPLITUDE));
        } catch (RuntimeException e) {
            Log.w("Meridian", "Could not tick", e);
        }
    }

    /** Held while the LAN world list listens; Wi-Fi drops incoming multicast without it. */
    private WifiManager.MulticastLock multicastLock;

    /** Takes (on) or gives back the Wi-Fi multicast lock. Counted, so overlapping listeners
     *  each pair one call of each. */
    public synchronized void multicast(boolean on) {
        try {
            if (multicastLock == null) {
                WifiManager wifi = (WifiManager) getApplicationContext()
                        .getSystemService(Context.WIFI_SERVICE);
                if (wifi == null) {
                    return;
                }
                multicastLock = wifi.createMulticastLock("meridian-lan");
            }
            if (on) {
                multicastLock.acquire();
            } else if (multicastLock.isHeld()) {
                multicastLock.release();
            }
        } catch (RuntimeException e) {
            Log.w("Meridian", "Could not change the multicast lock", e);
        }
    }

    /** The sensors, for the gyroscope the touch controls can aim with. */
    private SensorManager sensors;
    /** What the gyroscope turned since the game last took it: radians about the device's own x
     *  and y (those of its natural portrait). */
    private final float[] gyroTurned = new float[2];
    /** The last gyroscope event's time (ns), 0 before the first one since it started. */
    private long gyroLast;
    private final SensorEventListener gyroListener = new SensorEventListener() {
        @Override
        public void onSensorChanged(SensorEvent event) {
            long last = gyroLast;
            gyroLast = event.timestamp;
            float seconds = (event.timestamp - last) * 1e-9f;
            // The first event, or one after a gap: nothing to integrate over.
            if (last == 0 || seconds <= 0 || seconds > 0.1f) {
                return;
            }
            synchronized (gyroTurned) {
                gyroTurned[0] += event.values[0] * seconds;
                gyroTurned[1] += event.values[1] * seconds;
            }
        }

        @Override
        public void onAccuracyChanged(Sensor sensor, int accuracy) {
        }
    };

    /** Start or stop listening to the gyroscope; false when the phone has none. Adapted from
     *  Starling's gyroscope. */
    public boolean gyroscope(boolean on) {
        if (sensors == null) {
            sensors = (SensorManager) getSystemService(Context.SENSOR_SERVICE);
        }
        Sensor gyro = sensors == null ? null : sensors.getDefaultSensor(Sensor.TYPE_GYROSCOPE);
        if (gyro == null) {
            return false;
        }
        Log.d("MeridianHaptics", "gyroscope " + (on ? "on" : "off"));
        if (on) {
            gyroLast = 0;
            // Every 5 ms (200 Hz, the most an app gets without asking for more): a frame at
            // 120 fps still has a turn to take.
            sensors.registerListener(gyroListener, gyro, 5000);
        } else {
            sensors.unregisterListener(gyroListener);
        }
        synchronized (gyroTurned) {
            gyroTurned[0] = 0;
            gyroTurned[1] = 0;
        }
        return true;
    }

    /** What the gyroscope turned since the last call (radians about the device's x and y) and
     *  the display's rotation (Surface.ROTATION_*); it counts again from there. */
    @SuppressWarnings("deprecation")
    public float[] takeGyro() {
        int rotation = Build.VERSION.SDK_INT >= 30
                ? getDisplay().getRotation()
                : getWindowManager().getDefaultDisplay().getRotation();
        synchronized (gyroTurned) {
            float[] taken = {gyroTurned[0], gyroTurned[1], rotation};
            gyroTurned[0] = 0;
            gyroTurned[1] = 0;
            return taken;
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == PICK_IMPORT) {
            final String dest = pendingImportDest;
            final long maxBytes = pendingImportMaxBytes;
            if (dest == null) {
                return;
            }
            if (result != RESULT_OK || data == null || data.getData() == null) {
                new Thread(() -> {
                    try {
                        new File(getFilesDir(), dest + ".cancelled").createNewFile();
                    } catch (Exception e) {
                        Log.w("Meridian", "Could not mark import cancelled", e);
                    }
                }, "import-cancel").start();
                return;
            }
            final Uri uri = data.getData();
            new Thread(() -> {
                String displayName = dest;
                try (Cursor cursor = getContentResolver().query(
                        uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
                    if (cursor != null && cursor.moveToFirst()) {
                        String name = cursor.getString(0);
                        if (name != null && !name.isEmpty()) {
                            displayName = name;
                        }
                    }
                } catch (Exception e) {
                    Log.w("Meridian", "Could not query display name", e);
                }
                File target = new File(getFilesDir(), dest);
                File part = new File(getFilesDir(), dest + ".part");
                File nameFile = new File(getFilesDir(), dest + ".name");
                try (InputStream in = getContentResolver().openInputStream(uri);
                     OutputStream out = new FileOutputStream(part)) {
                    if (in == null) {
                        part.delete();
                        new File(getFilesDir(), dest + ".cancelled").createNewFile();
                        return;
                    }
                    byte[] buffer = new byte[64 * 1024];
                    long total = 0;
                    for (int n; total <= maxBytes && (n = in.read(buffer)) > 0; total += n) {
                        int chunk = (int) Math.min(n, maxBytes + 1 - total);
                        out.write(buffer, 0, chunk);
                    }
                } catch (Exception e) {
                    Log.w("Meridian", "Could not copy chosen import", e);
                    part.delete();
                    try {
                        new File(getFilesDir(), dest + ".cancelled").createNewFile();
                    } catch (Exception ignored) {
                    }
                    return;
                }
                try (OutputStream out = new FileOutputStream(nameFile)) {
                    out.write(displayName.getBytes(StandardCharsets.UTF_8));
                } catch (Exception e) {
                    Log.w("Meridian", "Could not write import display name", e);
                }
                if (!part.renameTo(target)) {
                    part.delete();
                }
            }, "import-copy").start();
            return;
        }
        if (request != PICK_SKIN || result != RESULT_OK || data == null || data.getData() == null) {
            return;
        }
        Uri uri = data.getData();
        // Off the UI thread; written beside the target and renamed, so the launcher never reads
        // half a file.
        new Thread(() -> {
            File target = new File(getFilesDir(), "picked-skin.png");
            File part = new File(getFilesDir(), "picked-skin.png.part");
            try (InputStream in = getContentResolver().openInputStream(uri);
                 OutputStream out = new FileOutputStream(part)) {
                byte[] buffer = new byte[64 * 1024];
                int total = 0;
                for (int n; total <= MAX_SKIN && (n = in.read(buffer)) > 0; total += n) {
                    out.write(buffer, 0, Math.min(n, MAX_SKIN + 1 - total));
                }
            } catch (Exception e) {
                Log.w("Meridian", "Could not copy the chosen skin", e);
                part.delete();
                return;
            }
            if (!part.renameTo(target)) {
                part.delete();
            }
        }, "skin-copy").start();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
        // Android drops the capture with the focus, and the game still holds the mouse.
        // ponytail: a capture Android takes back while focused (a system shortcut) stays lost
        // until the next focus change; add Starling's onPointerCaptureLost if players hit it.
        if (hasFocus && pointerWanted) {
            getWindow().getDecorView().requestPointerCapture();
        }
    }
}
