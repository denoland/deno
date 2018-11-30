// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.build;

import android.content.Context;
import android.content.res.AssetManager;
import android.content.res.Resources;

/**
 * All Java targets that require android have dependence on this class. Add methods that do not
 * require Android to {@link BuildHooks}.
 *
 * This class provides hooks needed when bytecode rewriting. Static convenience methods are used to
 * minimize the amount of code required to be manually generated when bytecode rewriting.
 *
 * This class contains default implementations for all methods and is used when no other
 * implementation is supplied to an android_apk target (via build_hooks_android_impl_deps).
 */
public abstract class BuildHooksAndroid {
    private static BuildHooksAndroid sInstance;

    private static BuildHooksAndroid get() {
        if (sInstance == null) {
            sInstance = constructBuildHooksAndroidImpl();
        }
        return sInstance;
    }

    // Creates an instance of BuildHooksAndroidImpl using reflection. Why is this necessary?
    // The downstream version of BuildHooksAndroidImpl pulls a bunch of methods into the main dex
    // that don't actually need to be there. This happens because there are @MainDex classes that
    // have Context methods added (via. bytecode rewriting) that call into BuildHooksAndroid.
    // Creating the instance via. reflection tricks proguard into thinking BuildHooksAndroidImpl
    // doesn't need to be in the main dex file.
    private static BuildHooksAndroid constructBuildHooksAndroidImpl() {
        try {
            // Not final to avoid inlining. Without this proguard is able to figure out that
            // BuildHooksAndroidImpl is actually used.
            String implClazzName = "org.chromium.build.BuildHooksAndroidImpl";
            Class<?> implClazz = Class.forName(implClazzName);
            return (BuildHooksAndroid) implClazz.newInstance();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    public static Resources getResources(Context context) {
        return get().getResourcesImpl(context);
    }

    protected abstract Resources getResourcesImpl(Context context);

    public static AssetManager getAssets(Context context) {
        return get().getAssetsImpl(context);
    }

    protected abstract AssetManager getAssetsImpl(Context context);

    public static Resources.Theme getTheme(Context context) {
        return get().getThemeImpl(context);
    }

    protected abstract Resources.Theme getThemeImpl(Context context);

    public static void setTheme(Context context, int theme) {
        get().setThemeImpl(context, theme);
    }

    protected abstract void setThemeImpl(Context context, int theme);

    public static Context createConfigurationContext(Context context) {
        return get().createConfigurationContextImpl(context);
    }

    protected abstract Context createConfigurationContextImpl(Context context);

    public static int getIdentifier(
            Resources resources, String name, String defType, String defPackage) {
        return get().getIdentifierImpl(resources, name, defType, defPackage);
    }

    protected abstract int getIdentifierImpl(
            Resources resources, String name, String defType, String defPackage);

    public static boolean isEnabled() {
        return get().isEnabledImpl();
    }

    protected abstract boolean isEnabledImpl();

    public static void initCustomResources(Context context) {
        get().initCustomResourcesImpl(context);
    }

    protected abstract void initCustomResourcesImpl(Context context);

    /**
     * Record custom resources related UMA. Requires native library to be loaded.
     */
    public static void maybeRecordResourceMetrics() {
        get().maybeRecordResourceMetricsImpl();
    }

    protected abstract void maybeRecordResourceMetricsImpl();
}