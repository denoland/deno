// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.incrementalinstall;

import android.annotation.SuppressLint;
import android.content.Context;
import android.os.Build;
import android.os.Process;
import android.util.Log;

import dalvik.system.DexFile;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.util.List;

/**
 * Provides the ability to add native libraries and .dex files to an existing class loader.
 * Tested with Jellybean MR2 - Marshmellow.
 */
final class ClassLoaderPatcher {
    private static final String TAG = "cr.incrementalinstall";
    private final File mAppFilesSubDir;
    private final ClassLoader mClassLoader;
    private final Object mLibcoreOs;
    private final int mProcessUid;
    final boolean mIsPrimaryProcess;

    ClassLoaderPatcher(Context context) throws ReflectiveOperationException {
        mAppFilesSubDir =
                new File(context.getApplicationInfo().dataDir, "incremental-install-files");
        mClassLoader = context.getClassLoader();
        mLibcoreOs = Reflect.getField(Class.forName("libcore.io.Libcore"), "os");
        mProcessUid = Process.myUid();
        mIsPrimaryProcess = context.getApplicationInfo().uid == mProcessUid;
        Log.i(TAG, "uid=" + mProcessUid + " (isPrimary=" + mIsPrimaryProcess + ")");
    }

    /**
     * Loads all dex files within |dexDir| into the app's ClassLoader.
     */
    @SuppressLint({
            "SetWorldReadable", "SetWorldWritable",
    })
    DexFile[] loadDexFiles(File dexDir) throws ReflectiveOperationException, IOException {
        Log.i(TAG, "Installing dex files from: " + dexDir);

        // The optimized dex files will be owned by this process' user.
        // Store them within the app's data dir rather than on /data/local/tmp
        // so that they are still deleted (by the OS) when we uninstall
        // (even on a non-rooted device).
        File incrementalDexesDir = new File(mAppFilesSubDir, "optimized-dexes");
        File isolatedDexesDir = new File(mAppFilesSubDir, "isolated-dexes");
        File optimizedDir;

        // In O, optimizedDirectory is ignored, and the files are always put in an "oat"
        // directory that is a sibling to the dex files themselves. SELinux policies
        // prevent using odex files from /data/local/tmp, so we must first copy them
        // into the app's data directory in order to get the odex files to live there.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            safeCopyAllFiles(dexDir, incrementalDexesDir);
            dexDir = incrementalDexesDir;
        }

        File[] dexFilesArr = dexDir.listFiles((File f) -> f.isFile()); // Ignore "oat" directory.
        if (dexFilesArr == null) {
            throw new FileNotFoundException("Dex dir does not exist: " + dexDir);
        }

        if (mIsPrimaryProcess) {
            ensureAppFilesSubDirExists();
            // Allows isolated processes to access the same files.
            incrementalDexesDir.mkdir();
            incrementalDexesDir.setReadable(true, false);
            incrementalDexesDir.setExecutable(true, false);
            // Create a directory for isolated processes to create directories in.
            isolatedDexesDir.mkdir();
            isolatedDexesDir.setWritable(true, false);
            isolatedDexesDir.setExecutable(true, false);

            optimizedDir = incrementalDexesDir;
        } else {
            // There is a UID check of the directory in dalvik.system.DexFile():
            // https://android.googlesource.com/platform/libcore/+/45e0260/dalvik/src/main/java/dalvik/system/DexFile.java#101
            // Rather than have each isolated process run DexOpt though, we use
            // symlinks within the directory to point at the browser process'
            // optimized dex files.
            optimizedDir = new File(isolatedDexesDir, "isolated-" + mProcessUid);
            optimizedDir.mkdir();
            // Always wipe it out and re-create for simplicity.
            Log.i(TAG, "Creating dex file symlinks for isolated process");
            for (File f : optimizedDir.listFiles()) {
                f.delete();
            }
            for (File f : incrementalDexesDir.listFiles()) {
                String to = "../../" + incrementalDexesDir.getName() + "/" + f.getName();
                File from = new File(optimizedDir, f.getName());
                createSymlink(to, from);
            }
        }

        Log.i(TAG, "Code cache dir: " + optimizedDir);
        Log.i(TAG, "Loading " + dexFilesArr.length + " dex files");

        Object dexPathList = Reflect.getField(mClassLoader, "pathList");
        Object[] dexElements = (Object[]) Reflect.getField(dexPathList, "dexElements");
        dexElements = addDexElements(dexFilesArr, optimizedDir, dexElements);
        Reflect.setField(dexPathList, "dexElements", dexElements);

        DexFile[] ret = new DexFile[dexElements.length];
        for (int i = 0; i < ret.length; ++i) {
            ret[i] = (DexFile) Reflect.getField(dexElements[i], "dexFile");
        }
        return ret;
    }

    /**
     * Sets up all libraries within |libDir| to be loadable by System.loadLibrary().
     */
    @SuppressLint("SetWorldReadable")
    void importNativeLibs(File libDir) throws ReflectiveOperationException, IOException {
        Log.i(TAG, "Importing native libraries from: " + libDir);
        if (!libDir.exists()) {
            Log.i(TAG, "No native libs exist.");
            return;
        }
        // The library copying is not necessary on older devices, but we do it anyways to
        // simplify things (it's fast compared to dexing).
        // https://code.google.com/p/android/issues/detail?id=79480
        File localLibsDir = new File(mAppFilesSubDir, "lib");
        safeCopyAllFiles(libDir, localLibsDir);
        addNativeLibrarySearchPath(localLibsDir);
    }

    @SuppressLint("SetWorldReadable")
    private void safeCopyAllFiles(File srcDir, File dstDir) throws IOException {
        // The library copying is not necessary on older devices, but we do it anyways to
        // simplify things (it's fast compared to dexing).
        // https://code.google.com/p/android/issues/detail?id=79480
        File lockFile = new File(mAppFilesSubDir, dstDir.getName() + ".lock");
        if (mIsPrimaryProcess) {
            ensureAppFilesSubDirExists();
            LockFile lock = LockFile.acquireRuntimeLock(lockFile);
            if (lock == null) {
                LockFile.waitForRuntimeLock(lockFile, 10 * 1000);
            } else {
                try {
                    dstDir.mkdir();
                    dstDir.setReadable(true, false);
                    dstDir.setExecutable(true, false);
                    copyChangedFiles(srcDir, dstDir);
                } finally {
                    lock.release();
                }
            }
        } else {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                // TODO: Work around this issue by using APK splits to install each dex / lib.
                throw new RuntimeException("Incremental install does not work on Android M+ "
                        + "with isolated processes. Use the gn arg:\n"
                        + "    disable_incremental_isolated_processes=true\n"
                        + "and try again.");
            }
            // Other processes: Waits for primary process to finish copying.
            LockFile.waitForRuntimeLock(lockFile, 10 * 1000);
        }
    }

    @SuppressWarnings("unchecked")
    private void addNativeLibrarySearchPath(File nativeLibDir) throws ReflectiveOperationException {
        Object dexPathList = Reflect.getField(mClassLoader, "pathList");
        Object currentDirs = Reflect.getField(dexPathList, "nativeLibraryDirectories");
        File[] newDirs = new File[] { nativeLibDir };
        // Switched from an array to an ArrayList in Lollipop.
        if (currentDirs instanceof List) {
            List<File> dirsAsList = (List<File>) currentDirs;
            dirsAsList.add(0, nativeLibDir);
        } else {
            File[] dirsAsArray = (File[]) currentDirs;
            Reflect.setField(dexPathList, "nativeLibraryDirectories",
                    Reflect.concatArrays(newDirs, newDirs, dirsAsArray));
        }

        Object[] nativeLibraryPathElements;
        try {
            nativeLibraryPathElements =
                    (Object[]) Reflect.getField(dexPathList, "nativeLibraryPathElements");
        } catch (NoSuchFieldException e) {
            // This field doesn't exist pre-M.
            return;
        }
        Object[] additionalElements = makeNativePathElements(newDirs);
        Reflect.setField(dexPathList, "nativeLibraryPathElements",
                Reflect.concatArrays(nativeLibraryPathElements, additionalElements,
                        nativeLibraryPathElements));
    }

    private static void copyChangedFiles(File srcDir, File dstDir) throws IOException {
        // No need to delete stale libs since libraries are loaded explicitly.
        int numNotChanged = 0;
        for (File f : srcDir.listFiles()) {
            // Note: Tried using hardlinks, but resulted in EACCES exceptions.
            File dest = new File(dstDir, f.getName());
            if (!copyIfModified(f, dest)) {
                numNotChanged++;
            }
        }
        if (numNotChanged > 0) {
            Log.i(TAG, numNotChanged + " libs already up to date.");
        }
    }

    @SuppressLint("SetWorldReadable")
    private static boolean copyIfModified(File src, File dest) throws IOException {
        long lastModified = src.lastModified();
        if (dest.exists() && dest.lastModified() == lastModified) {
            return false;
        }
        Log.i(TAG, "Copying " + src + " -> " + dest);
        FileInputStream istream = new FileInputStream(src);
        FileOutputStream ostream = new FileOutputStream(dest);
        ostream.getChannel().transferFrom(istream.getChannel(), 0, istream.getChannel().size());
        istream.close();
        ostream.close();
        dest.setReadable(true, false);
        dest.setExecutable(true,  false);
        dest.setLastModified(lastModified);
        return true;
    }

    private void ensureAppFilesSubDirExists() {
        mAppFilesSubDir.mkdir();
        mAppFilesSubDir.setExecutable(true, false);
    }

    private void createSymlink(String to, File from) throws ReflectiveOperationException {
        Reflect.invokeMethod(mLibcoreOs, "symlink", to, from.getAbsolutePath());
    }

    private static Object[] makeNativePathElements(File[] paths)
            throws ReflectiveOperationException {
        Object[] entries = new Object[paths.length];
        if (Build.VERSION.SDK_INT >= 26) {
            Class<?> entryClazz = Class.forName("dalvik.system.DexPathList$NativeLibraryElement");
            for (int i = 0; i < paths.length; ++i) {
                entries[i] = Reflect.newInstance(entryClazz, paths[i]);
            }
        } else {
            Class<?> entryClazz = Class.forName("dalvik.system.DexPathList$Element");
            for (int i = 0; i < paths.length; ++i) {
                entries[i] = Reflect.newInstance(entryClazz, paths[i], true, null, null);
            }
        }
        return entries;
    }

    private Object[] addDexElements(File[] files, File optimizedDirectory, Object[] curDexElements)
            throws ReflectiveOperationException {
        Class<?> entryClazz = Class.forName("dalvik.system.DexPathList$Element");
        Class<?> clazz = Class.forName("dalvik.system.DexPathList");
        Object[] ret =
                Reflect.concatArrays(curDexElements, curDexElements, new Object[files.length]);
        File emptyDir = new File("");
        for (int i = 0; i < files.length; ++i) {
            File file = files[i];
            Object dexFile;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                // loadDexFile requires that ret contain all previously added elements.
                dexFile = Reflect.invokeMethod(clazz, "loadDexFile", file, optimizedDirectory,
                                               mClassLoader, ret);
            } else {
                dexFile = Reflect.invokeMethod(clazz, "loadDexFile", file, optimizedDirectory);
            }
            Object dexElement;
            if (Build.VERSION.SDK_INT >= 26) {
                dexElement = Reflect.newInstance(entryClazz, dexFile, file);
            } else {
                dexElement = Reflect.newInstance(entryClazz, emptyDir, false, file, dexFile);
            }
            ret[curDexElements.length + i] = dexElement;
        }
        return ret;
    }
}
