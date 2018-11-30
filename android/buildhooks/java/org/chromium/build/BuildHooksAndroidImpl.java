// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.build;

import android.content.Context;
import android.content.res.AssetManager;
import android.content.res.Resources;

/**
 * Instantiatable version of {@link BuildHooksAndroid} with dummy implementations.
 */
public class BuildHooksAndroidImpl extends BuildHooksAndroid {
    @Override
    protected final Resources getResourcesImpl(Context context) {
        return null;
    }

    @Override
    protected AssetManager getAssetsImpl(Context context) {
        return null;
    }

    @Override
    protected Resources.Theme getThemeImpl(Context context) {
        return null;
    }

    @Override
    protected void setThemeImpl(Context context, int theme) {}

    @Override
    protected Context createConfigurationContextImpl(Context context) {
        return null;
    }

    @Override
    protected int getIdentifierImpl(
            Resources resources, String name, String defType, String defPackage) {
        return resources.getIdentifier(name, defType, defPackage);
    }

    @Override
    protected boolean isEnabledImpl() {
        return false;
    }

    @Override
    protected void initCustomResourcesImpl(Context context) {}

    @Override
    protected void maybeRecordResourceMetricsImpl() {}
}
