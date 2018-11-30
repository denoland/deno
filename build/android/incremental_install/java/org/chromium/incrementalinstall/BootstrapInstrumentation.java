// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.incrementalinstall;

import android.app.Instrumentation;
import android.os.Bundle;

/**
 * Notifies BootstrapApplication of the call to Instrumentation.onCreate().
 */
public final class BootstrapInstrumentation extends Instrumentation {
    private final BootstrapApplication mApp;

    BootstrapInstrumentation(BootstrapApplication app) {
        mApp = app;
    }

    @Override
    public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        mApp.onInstrumentationCreate(arguments);
    }
}
