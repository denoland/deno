// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.incrementalinstall;

import android.app.Application;
import android.app.Instrumentation;
import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.PackageManager.NameNotFoundException;
import android.os.Bundle;
import android.util.Log;

import dalvik.system.DexFile;

import java.io.File;
import java.lang.ref.WeakReference;
import java.util.List;
import java.util.Map;

/**
 * An Application that replaces itself with another Application (as defined in
 * an AndroidManifext.xml meta-data tag). It loads the other application only
 * after side-loading its .so and .dex files from /data/local/tmp.
 *
 * This class is highly dependent on the private implementation details of
 * Android's ActivityThread.java. However, it has been tested to work with
 * JellyBean through Marshmallow.
 */
public final class BootstrapApplication extends Application {
    private static final String TAG = "cr.incrementalinstall";
    private static final String MANAGED_DIR_PREFIX = "/data/local/tmp/incremental-app-";
    private static final String REAL_APP_META_DATA_NAME = "incremental-install-real-app";
    private static final String REAL_INSTRUMENTATION_META_DATA_NAME0 =
            "incremental-install-real-instrumentation-0";
    private static final String REAL_INSTRUMENTATION_META_DATA_NAME1 =
            "incremental-install-real-instrumentation-1";

    private ClassLoaderPatcher mClassLoaderPatcher;
    private Application mRealApplication;
    private Instrumentation mOrigInstrumentation;
    private Instrumentation mRealInstrumentation;
    private Object mStashedProviderList;
    private Object mActivityThread;
    public static DexFile[] sIncrementalDexFiles; // Needed by junit test runner.

    @Override
    protected void attachBaseContext(Context context) {
        super.attachBaseContext(context);
        try {
            mActivityThread = Reflect.invokeMethod(Class.forName("android.app.ActivityThread"),
                    "currentActivityThread");
            mClassLoaderPatcher = new ClassLoaderPatcher(context);

            mOrigInstrumentation =
                    (Instrumentation) Reflect.getField(mActivityThread, "mInstrumentation");
            Context instContext = mOrigInstrumentation.getContext();
            if (instContext == null) {
                instContext = context;
            }

            // When running with an instrumentation that lives in a different package from the
            // application, we must load the dex files and native libraries from both pacakges.
            // This logic likely won't work when the instrumentation is incremental, but the app is
            // non-incremental. This configuration isn't used right now though.
            String appPackageName = getPackageName();
            String instPackageName = instContext.getPackageName();
            boolean instPackageNameDiffers = !appPackageName.equals(instPackageName);
            Log.i(TAG, "App PackageName: " + appPackageName);
            if (instPackageNameDiffers) {
                Log.i(TAG, "Inst PackageName: " + instPackageName);
            }

            File appIncrementalRootDir = new File(MANAGED_DIR_PREFIX + appPackageName);
            File appLibDir = new File(appIncrementalRootDir, "lib");
            File appDexDir = new File(appIncrementalRootDir, "dex");
            File appInstallLockFile = new File(appIncrementalRootDir, "install.lock");
            File appFirstRunLockFile = new File(appIncrementalRootDir, "firstrun.lock");
            File instIncrementalRootDir = new File(MANAGED_DIR_PREFIX + instPackageName);
            File instLibDir = new File(instIncrementalRootDir, "lib");
            File instDexDir = new File(instIncrementalRootDir, "dex");
            File instInstallLockFile = new File(instIncrementalRootDir, "install.lock");
            File instFirstRunLockFile = new File(instIncrementalRootDir, "firstrun.lock");

            boolean isFirstRun = LockFile.installerLockExists(appFirstRunLockFile)
                    || (instPackageNameDiffers
                               && LockFile.installerLockExists(instFirstRunLockFile));
            if (isFirstRun) {
                if (mClassLoaderPatcher.mIsPrimaryProcess) {
                    // Wait for incremental_install.py to finish.
                    LockFile.waitForInstallerLock(appInstallLockFile, 30 * 1000);
                    LockFile.waitForInstallerLock(instInstallLockFile, 30 * 1000);
                } else {
                    // Wait for the browser process to create the optimized dex files
                    // and copy the library files.
                    LockFile.waitForInstallerLock(appFirstRunLockFile, 60 * 1000);
                    LockFile.waitForInstallerLock(instFirstRunLockFile, 60 * 1000);
                }
            }

            mClassLoaderPatcher.importNativeLibs(instLibDir);
            sIncrementalDexFiles = mClassLoaderPatcher.loadDexFiles(instDexDir);
            if (instPackageNameDiffers) {
                mClassLoaderPatcher.importNativeLibs(appLibDir);
                mClassLoaderPatcher.loadDexFiles(appDexDir);
            }

            if (isFirstRun && mClassLoaderPatcher.mIsPrimaryProcess) {
                LockFile.clearInstallerLock(appFirstRunLockFile);
                if (instPackageNameDiffers) {
                    LockFile.clearInstallerLock(instFirstRunLockFile);
                }
            }

            // mInstrumentationAppDir is one of a set of fields that is initialized only when
            // instrumentation is active.
            if (Reflect.getField(mActivityThread, "mInstrumentationAppDir") != null) {
                String metaDataName = REAL_INSTRUMENTATION_META_DATA_NAME0;
                if (mOrigInstrumentation instanceof SecondInstrumentation) {
                    metaDataName = REAL_INSTRUMENTATION_META_DATA_NAME1;
                }
                initInstrumentation(getClassNameFromMetadata(metaDataName, instContext));
            } else {
                Log.i(TAG, "No instrumentation active.");
            }

            // Even when instrumentation is not enabled, ActivityThread uses a default
            // Instrumentation instance internally. We hook it here in order to hook into the
            // call to Instrumentation.onCreate().
            Reflect.setField(mActivityThread, "mInstrumentation",
                    new BootstrapInstrumentation(this));

            // attachBaseContext() is called from ActivityThread#handleBindApplication() and
            // Application#mApplication is changed right after we return. Thus, we cannot swap
            // the Application instances until onCreate() is called.
            String realApplicationName = getClassNameFromMetadata(REAL_APP_META_DATA_NAME, context);
            Log.i(TAG, "Instantiating " + realApplicationName);
            mRealApplication =
                    (Application) Reflect.newInstance(Class.forName(realApplicationName));
            Reflect.invokeMethod(mRealApplication, "attachBaseContext", context);

            // Between attachBaseContext() and onCreate(), ActivityThread tries to instantiate
            // all ContentProviders. The ContentProviders break without the correct Application
            // class being installed, so temporarily pretend there are no providers, and then
            // instantiate them explicitly within onCreate().
            disableContentProviders();
            Log.i(TAG, "Waiting for Instrumentation.onCreate");
        } catch (Exception e) {
            throw new RuntimeException("Incremental install failed.", e);
        }
    }

    /**
     * Returns the fully-qualified class name for the given key, stored in a
     * &lt;meta&gt; witin the manifest.
     */
    private static String getClassNameFromMetadata(String key, Context context)
            throws NameNotFoundException {
        String pkgName = context.getPackageName();
        ApplicationInfo appInfo = context.getPackageManager().getApplicationInfo(pkgName,
                PackageManager.GET_META_DATA);
        String value = appInfo.metaData.getString(key);
        if (value != null && !value.contains(".")) {
            value = pkgName + "." + value;
        }
        return value;
    }

    /**
     * Instantiates and initializes mRealInstrumentation (the real Instrumentation class).
     */
    private void initInstrumentation(String realInstrumentationName)
            throws ReflectiveOperationException {
        if (realInstrumentationName == null) {
            // This is the case when an incremental app is used as a target for an instrumentation
            // test. In this case, ActivityThread can instantiate the proper class just fine since
            // it exists within the test apk (as opposed to the incremental apk-under-test).
            Log.i(TAG, "Running with external instrumentation");
            mRealInstrumentation = mOrigInstrumentation;
            return;
        }
        // For unit tests, the instrumentation class is replaced in the manifest by a build step
        // because ActivityThread tries to instantiate it before we get a chance to load the
        // incremental dex files.
        Log.i(TAG, "Instantiating instrumentation " + realInstrumentationName);
        mRealInstrumentation = (Instrumentation) Reflect.newInstance(
                Class.forName(realInstrumentationName));

        // Initialize the fields that are set by Instrumentation.init().
        String[] initFields = {"mAppContext", "mComponent", "mInstrContext", "mMessageQueue",
                "mThread", "mUiAutomationConnection", "mWatcher"};
        for (String fieldName : initFields) {
            Reflect.setField(mRealInstrumentation, fieldName,
                    Reflect.getField(mOrigInstrumentation, fieldName));
        }
    }

    /**
     * Called by BootstrapInstrumentation from Instrumentation.onCreate().
     * This happens regardless of whether or not instrumentation is enabled.
     */
    void onInstrumentationCreate(Bundle arguments) {
        Log.i(TAG, "Instrumentation.onCreate() called. Swapping references.");
        try {
            swapApplicationReferences();
            enableContentProviders();
            if (mRealInstrumentation != null) {
                Reflect.setField(mActivityThread, "mInstrumentation", mRealInstrumentation);
                mRealInstrumentation.onCreate(arguments);
            }
        } catch (Exception e) {
            throw new RuntimeException("Incremental install failed.", e);
        }
    }

    @Override
    public void onCreate() {
        super.onCreate();
        try {
            Log.i(TAG, "Application.onCreate() called.");
            mRealApplication.onCreate();
        } catch (Exception e) {
            throw new RuntimeException("Incremental install failed.", e);
        }
    }

    /**
     * Nulls out ActivityThread.mBoundApplication.providers.
     */
    private void disableContentProviders() throws ReflectiveOperationException {
        Object data = Reflect.getField(mActivityThread, "mBoundApplication");
        mStashedProviderList = Reflect.getField(data, "providers");
        Reflect.setField(data, "providers", null);
    }

    /**
     * Restores the value of ActivityThread.mBoundApplication.providers, and invokes
     * ActivityThread#installContentProviders().
     */
    private void enableContentProviders() throws ReflectiveOperationException {
        Object data = Reflect.getField(mActivityThread, "mBoundApplication");
        Reflect.setField(data, "providers", mStashedProviderList);
        if (mStashedProviderList != null && mClassLoaderPatcher.mIsPrimaryProcess) {
            Log.i(TAG, "Instantiating content providers");
            Reflect.invokeMethod(mActivityThread, "installContentProviders", mRealApplication,
                    mStashedProviderList);
        }
        mStashedProviderList = null;
    }

    /**
     * Changes all fields within framework classes that have stored an reference to this
     * BootstrapApplication to instead store references to mRealApplication.
     * @throws NoSuchFieldException
     */
    @SuppressWarnings("unchecked")
    private void swapApplicationReferences() throws ReflectiveOperationException {
        if (Reflect.getField(mActivityThread, "mInitialApplication") == this) {
            Reflect.setField(mActivityThread, "mInitialApplication", mRealApplication);
        }

        List<Application> allApplications =
                (List<Application>) Reflect.getField(mActivityThread, "mAllApplications");
        for (int i = 0; i < allApplications.size(); i++) {
            if (allApplications.get(i) == this) {
                allApplications.set(i, mRealApplication);
            }
        }

        // Contains a reference to BootstrapApplication and will cause BroadCastReceivers to fail
        // if not replaced.
        Object contextWrapperBase = Reflect.getField(mRealApplication, "mBase");
        Reflect.setField(contextWrapperBase, "mOuterContext", mRealApplication);

        for (String fieldName : new String[] {"mPackages", "mResourcePackages"}) {
            Map<String, WeakReference<?>> packageMap =
                    (Map<String, WeakReference<?>>) Reflect.getField(mActivityThread, fieldName);
            for (Map.Entry<String, WeakReference<?>> entry : packageMap.entrySet()) {
                Object loadedApk = entry.getValue().get();
                if (loadedApk != null && Reflect.getField(loadedApk, "mApplication") == this) {
                    Reflect.setField(loadedApk, "mApplication", mRealApplication);
                    Reflect.setField(mRealApplication, "mLoadedApk", loadedApk);
                }
            }
        }
    }
}
