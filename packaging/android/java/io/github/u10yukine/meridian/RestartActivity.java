// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
package io.github.u10yukine.meridian;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Process;
import android.os.SystemClock;
import java.io.File;

/**
 * In a process of its own: waits for the app's process to end, then starts MeridianActivity in a
 * fresh one. The launcher and the game each run in their own process (winit makes one event loop
 * per process), so every switch between them comes through here. Adapted from Starling's
 * LauncherActivity (packaging/android/java) at 5432370d5736.
 */
public class RestartActivity extends Activity {
    /** The process to wait for. */
    static final String AFTER = "after";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        final int pid = getIntent().getIntExtra(AFTER, 0);
        new Thread(() -> {
            // It ends itself right after asking; after 3 s it is ended here.
            for (int i = 0; i < 60 && alive(pid); i++) {
                SystemClock.sleep(50);
            }
            if (alive(pid)) {
                Process.killProcess(pid);
            }
            startActivity(new Intent(this, MeridianActivity.class)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
            runOnUiThread(() -> {
                finish();
                overridePendingTransition(0, 0);
                Process.killProcess(Process.myPid());
            });
        }).start();
    }

    private static boolean alive(int pid) {
        return pid > 0 && new File("/proc/" + pid).exists();
    }
}
