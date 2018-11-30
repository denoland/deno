// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.incrementalinstall;

import android.util.Log;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.channels.FileLock;
import java.util.concurrent.Callable;

/**
 * Helpers for dealing with .lock files used during install / first run.
 */
final class LockFile {
    private static final String TAG = "cr.incrementalinstall";

    private final File mFile;
    private final FileOutputStream mOutputStream;
    private final FileLock mFileLock;

    private LockFile(File file, FileOutputStream outputStream, FileLock fileLock) {
        mFile = file;
        mOutputStream = outputStream;
        mFileLock = fileLock;
    }

    /**
     * Clears the lock file by writing to it (making it non-zero in length);
     */
    static void clearInstallerLock(File lockFile) throws IOException {
        Log.i(TAG, "Clearing " + lockFile);
        // On Android M+, we can't delete files in /data/local/tmp, so we write to it instead.
        FileOutputStream os = new FileOutputStream(lockFile);
        os.write(1);
        os.close();
    }

    /**
     * Waits for the given file to be non-zero in length.
     */
    static void waitForInstallerLock(final File file, long timeoutMs) {
        pollingWait(new Callable<Boolean>() {
            @Override public Boolean call() {
                return !installerLockExists(file);
            }
        }, file, timeoutMs);
    }

    /**
     * Waits for the given file to be non-zero in length.
     */
    private static void pollingWait(Callable<Boolean> func, File file, long timeoutMs) {
        long pollIntervalMs = 200;
        for (int i = 0; i < timeoutMs / pollIntervalMs; i++) {
            try {
                if (func.call()) {
                    if (i > 0) {
                        Log.i(TAG, "Finished waiting on lock file: " + file);
                    }
                    return;
                } else if (i == 0) {
                    Log.i(TAG, "Waiting on lock file: " + file);
                }
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
            try {
                Thread.sleep(pollIntervalMs);
            } catch (InterruptedException e) {
                // Should never happen.
            }
        }
        throw new RuntimeException("Timed out waiting for lock file: " + file);
    }

    /**
     * Returns whether the given lock file is missing or is in the locked state.
     */
    static boolean installerLockExists(File file) {
        return !file.exists() || file.length() == 0;
    }

    /**
     * Attempts to acquire a lock for the given file.
     * @return Returns the FileLock if it was acquired, or null otherwise.
     */
    static LockFile acquireRuntimeLock(File file) {
        try {
            FileOutputStream outputStream = new FileOutputStream(file);
            FileLock lock = outputStream.getChannel().tryLock();
            if (lock != null) {
                Log.i(TAG, "Created lock file: " + file);
                return new LockFile(file, outputStream, lock);
            }
            outputStream.close();
        } catch (IOException e) {
            // Do nothing. We didn't get the lock.
            Log.w(TAG, "Exception trying to acquire lock " + file, e);
        }
        return null;
    }

    /**
     * Waits for the given file to not exist.
     */
    static void waitForRuntimeLock(final File file, long timeoutMs) {
        pollingWait(new Callable<Boolean>() {
            @Override public Boolean call() {
                return !file.exists();
            }
        }, file, timeoutMs);
    }

    /**
     * Releases and deletes the lock file.
     */
    void release() throws IOException {
        Log.i(TAG, "Deleting lock file: " + mFile);
        mFileLock.release();
        mOutputStream.close();
        if (!mFile.delete()) {
            throw new IOException("Failed to delete lock file: " + mFile);
        }
    }
}
