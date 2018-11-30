// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.build;

/**
 * All Java targets that support android have dependence on this class.
 */
public abstract class BuildHooks {
    /**
     * Defines an interface for reporting assertion error.
     */
    @FunctionalInterface
    public interface ReportAssertionCallback {
        void run(AssertionError arg);
    }

    private static ReportAssertionCallback sReportAssertionCallback;

    /**
     * This method is used to handle assert failures when asserts are enabled by
     * //build/android/bytecode:java_bytecode_rewriter. For non-release builds, this is always
     * enabled and assert failures will result in an assertion error being thrown. For release
     * builds, this is only enabled when report_java_assert = true. Assert failures will result in
     * an error report being uploaded to the crash servers only if the callback is set (so that this
     * can be a no-op for WebView in Monochrome). This also means that asserts hit before the
     * callback is set will be no-op's as well.
     */
    public static void assertFailureHandler(AssertionError assertionError) {
        if (BuildHooksConfig.REPORT_JAVA_ASSERT) {
            if (sReportAssertionCallback != null) {
                sReportAssertionCallback.run(assertionError);
            }
        } else {
            throw assertionError;
        }
    }

    /**
     * Set the callback function that handles assert failure.
     * This should be called from attachBaseContext.
     */
    public static void setReportAssertionCallback(ReportAssertionCallback callback) {
        if (!BuildHooksConfig.REPORT_JAVA_ASSERT) {
            throw new AssertionError();
        }
        sReportAssertionCallback = callback;
    }
}
