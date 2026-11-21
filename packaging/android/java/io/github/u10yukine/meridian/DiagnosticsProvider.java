// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
package io.github.u10yukine.meridian;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/**
 * Serves the diagnostic ZIPs in {@code files/diagnostics/} to the app the player shares one with
 * (Mail, Drive, Discord): a content:// URI, read-only, granted per share. Nothing else of the app's
 * storage is reachable through it. Written by hand: the build has no androidx FileProvider.
 */
public class DiagnosticsProvider extends ContentProvider {
    private static final String DIR = "diagnostics";
    private static final String PREFIX = "meridian-diagnostics-";

    /** The authority: the package's own, so a release build and a dev build can coexist. */
    private static String authority(Context context) {
        return context.getPackageName() + ".diagnostics";
    }

    /** The URI that serves {@code file}, or null when it isn't a diagnostic ZIP in the folder. */
    static Uri uriFor(Context context, File file) {
        File served = served(context, file.getName());
        try {
            if (served == null || !served.getCanonicalFile().equals(file.getCanonicalFile())) {
                return null;
            }
        } catch (IOException e) {
            return null;
        }
        return new Uri.Builder().scheme("content").authority(authority(context))
                .appendPath(file.getName()).build();
    }

    /** The file {@code name} names in the diagnostics folder, if it is one of our ZIPs. */
    private static File served(Context context, String name) {
        if (!name.startsWith(PREFIX) || !name.endsWith(".zip") || name.indexOf('/') >= 0) {
            return null;
        }
        try {
            File folder = new File(context.getFilesDir(), DIR).getCanonicalFile();
            File file = new File(folder, name).getCanonicalFile();
            return folder.equals(file.getParentFile()) && file.isFile() ? file : null;
        } catch (IOException e) {
            return null;
        }
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        File file = served(getContext(), uri.getLastPathSegment() == null ? "" : uri.getLastPathSegment());
        if (file == null || !"r".equals(mode)) {
            throw new FileNotFoundException(uri.toString());
        }
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
    }

    /** The name and size mail, drive and chat apps ask for. */
    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] args, String order) {
        File file = served(getContext(), uri.getLastPathSegment() == null ? "" : uri.getLastPathSegment());
        if (file == null) {
            return null;
        }
        String[] columns = projection != null ? projection
                : new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        Object[] row = new Object[columns.length];
        for (int i = 0; i < columns.length; i++) {
            row[i] = OpenableColumns.DISPLAY_NAME.equals(columns[i]) ? (Object) file.getName()
                    : OpenableColumns.SIZE.equals(columns[i]) ? (Object) file.length() : null;
        }
        MatrixCursor cursor = new MatrixCursor(columns, 1);
        cursor.addRow(row);
        return cursor;
    }

    @Override
    public String getType(Uri uri) {
        return "application/zip";
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException("read-only");
    }

    @Override
    public int delete(Uri uri, String selection, String[] args) {
        throw new UnsupportedOperationException("read-only");
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] args) {
        throw new UnsupportedOperationException("read-only");
    }
}
