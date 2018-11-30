// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.build;

import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.LineNumberReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;

import proguard.retrace.ReTrace;

/**
 * A wrapper around ReTrace that:
 *  1. Hardcodes a more useful line regular expression
 *  2. Disables output buffering
 */
public class FlushingReTrace {
    // E.g.: D/ConnectivityService(18029): Message
    // E.g.: W/GCM     ( 151): Message
    // E.g.: 09-08 14:22:59.995 18029 18055 I ProcessStatsService: Message
    // E.g.: 09-08 14:30:59.145 17731 18020 D MDnsDS  : Message
    private static final String LOGCAT_PREFIX =
            "(?:[VDIWEF]/.*?\\( *\\d+\\): |\\d\\d-\\d\\d [0-9:. ]+[VDIWEF] .*?: )?";

    // Note: Order of these sub-patterns defines their precedence.
    // Note: Deobfuscation of methods without the presense of line numbers basically never works.
    // There is a test for these pattern at //build/android/stacktrace/java_deobfuscate_test.py
    private static final String LINE_PARSE_REGEX =
            // Eagerly match logcat prefix to avoid conflicting with the patterns below.
            LOGCAT_PREFIX
            + "(?:"
            // Based on default ReTrace regex, but with "at" changed to to allow :
            // E.g.: 06-22 13:58:02.895  4674  4674 E THREAD_STATE:     bLA.a(PG:173)
            // Normal stack trace lines look like:
            // \tat org.chromium.chrome.browser.tab.Tab.handleJavaCrash(Tab.java:682)
            + "(?:.*?(?::|\\bat)\\s+%c\\.%m\\s*\\(%s(?::%l)?\\))|"
            // E.g.: VFY: unable to resolve new-instance 3810 (LSome/Framework/Class;) in Lfoo/Bar;
            + "(?:.*L%C;.*)|"
            // E.g.: END SomeTestClass#someMethod
            + "(?:.*?%c#%m.*?)|"
            // E.g.: The member "Foo.bar"
            // E.g.: The class "Foobar"
            + "(?:.*?\"%c\\.%m\".*)|"
            + "(?:.*?\"%c\".*)|"
            // Special-case for a common junit logcat message:
            // E.g.: java.lang.NoClassDefFoundError: SomeFrameworkClass in isTestClass for Foo
            + "(?:.* isTestClass for %c)|"
            // E.g.: java.lang.RuntimeException: Intentional Java Crash
            + "(?:%c:.*)|"
            // All lines that end with a class / class+method:
            // E.g.: The class: Foo
            // E.g.: INSTRUMENTATION_STATUS: class=Foo
            // E.g.: NoClassDefFoundError: SomeFrameworkClass in isTestClass for Foo
            // E.g.: Could not find class 'SomeFrameworkClass', referenced from method Foo.bar
            // E.g.: Could not find method SomeFrameworkMethod, referenced from method Foo.bar
            + "(?:.*(?:=|:\\s*|\\s+)%c\\.%m)|"
            + "(?:.*(?:=|:\\s*|\\s+)%c)"
            + ")";

    private static void usage() {
        System.err.println("Usage: echo $OBFUSCATED_CLASS | java_deobfuscate Foo.apk.mapping");
        System.err.println("Usage: java_deobfuscate Foo.apk.mapping < foo.log");
        System.err.println("Note: Deobfuscation of symbols outside the context of stack "
                + "traces will work only when lines match the regular expression defined "
                + "in FlushingReTrace.java.");
        System.err.println("Also: Deobfuscation of method names without associated line "
                + "numbers does not seem to work.");
        System.exit(1);
    }

    public static void main(String[] args) {
        if (args.length != 1 || args[0].startsWith("-")) {
            usage();
        }

        File mappingFile = new File(args[0]);
        try {
            LineNumberReader reader = new LineNumberReader(
                    new BufferedReader(new InputStreamReader(System.in, "UTF-8")));

            // Enabling autoFlush is the main difference from ReTrace.main().
            boolean autoFlush = true;
            PrintWriter writer =
                    new PrintWriter(new OutputStreamWriter(System.out, "UTF-8"), autoFlush);

            boolean verbose = false;
            new ReTrace(LINE_PARSE_REGEX, verbose, mappingFile).retrace(reader, writer);
        } catch (IOException ex) {
            // Print a verbose stack trace.
            ex.printStackTrace();
            System.exit(1);
        }

        System.exit(0);
    }
}
