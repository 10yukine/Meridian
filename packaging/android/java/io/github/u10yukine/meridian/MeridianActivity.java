// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
package io.github.u10yukine.meridian;

import android.app.NativeActivity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Process;
import android.util.Log;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

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

    private static final int PICK_SKIN = 1;
    /** The largest skin the launcher takes (meridian-launcher's skins::MAX_PNG); one byte more
     *  is copied, so a bigger file reads as too big rather than as a cut-off PNG. */
    private static final int MAX_SKIN = 1024 * 1024;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        hideSystemBars();
        // ponytail: Android 8 to 10 don't report the keyboard's height this way (0 there); read
        // getWindowVisibleDisplayFrame if players on them need the fields lifted.
        if (Build.VERSION.SDK_INT >= 30) {
            getWindow().getDecorView().setOnApplyWindowInsetsListener((view, insets) -> {
                imeHeight = insets.getInsets(WindowInsets.Type.ime()).bottom;
                return view.onApplyWindowInsets(insets);
            });
        }
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

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
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
