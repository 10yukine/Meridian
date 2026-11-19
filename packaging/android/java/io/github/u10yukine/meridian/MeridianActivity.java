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
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

/**
 * The game runs in libmeridian.so (NativeActivity); this only does what the NDK cannot: hides the
 * system bars, and captures the mouse. The game calls the public methods from its own thread
 * (meridian-android's activity.rs). Adapted from Starling's StarlingActivity
 * (packaging/android/java) at 5432370d5736.
 */
public class MeridianActivity extends NativeActivity {
    /** The game holds the mouse (capturePointer); on the UI thread only. */
    private boolean pointerWanted;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        hideSystemBars();
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
