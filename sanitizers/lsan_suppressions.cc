// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// This file contains the default suppressions for LeakSanitizer.
// You can also pass additional suppressions via LSAN_OPTIONS:
// LSAN_OPTIONS=suppressions=/path/to/suppressions. Please refer to
// http://dev.chromium.org/developers/testing/leaksanitizer for more info.

#if defined(LEAK_SANITIZER)

// Please make sure the code below declares a single string variable
// kLSanDefaultSuppressions which contains LSan suppressions delimited by
// newlines. See http://dev.chromium.org/developers/testing/leaksanitizer
// for the instructions on writing suppressions.
char kLSanDefaultSuppressions[] =
    // Intentional leak used as sanity test for Valgrind/memcheck.
    "leak:base::ToolsSanityTest_MemoryLeak_Test::TestBody\n"

    // ================ Leaks in third-party code ================

    // False positives in libfontconfig. http://crbug.com/39050
    "leak:libfontconfig\n"
    // eglibc-2.19/string/strdup.c creates false positive leak errors because of
    // the same reason as crbug.com/39050. The leak error stack trace, when
    // unwind on malloc, includes a call to libfontconfig. But the default stack
    // trace is too short in leak sanitizer bot to make the libfontconfig
    // suppression works. http://crbug.com/605286
    "leak:__strdup\n"

    // Leaks in Nvidia's libGL.
    "leak:libGL.so\n"

    // TODO(eugenis): revisit NSS suppressions after the switch to BoringSSL
    // NSS leaks in CertDatabaseNSSTest tests. http://crbug.com/51988
    "leak:net::NSSCertDatabase::ImportFromPKCS12\n"
    "leak:net::NSSCertDatabase::ListCerts\n"
    "leak:net::NSSCertDatabase::DeleteCertAndKey\n"
    "leak:crypto::ScopedTestNSSDB::ScopedTestNSSDB\n"
    // Another leak due to not shutting down NSS properly.
    // http://crbug.com/124445
    "leak:error_get_my_stack\n"
    // The NSS suppressions above will not fire when the fast stack unwinder is
    // used, because it can't unwind through NSS libraries. Apply blanket
    // suppressions for now.
    "leak:libnssutil3\n"
    "leak:libnspr4\n"
    "leak:libnss3\n"
    "leak:libplds4\n"
    "leak:libnssckbi\n"

    // XRandR has several one time leaks.
    "leak:libxrandr\n"

    // xrandr leak. http://crbug.com/119677
    "leak:XRRFindDisplay\n"

    // http://crbug.com/431213, http://crbug.com/416665
    "leak:gin/object_template_builder.h\n"

    // Leaks in swrast_dri.so. http://crbug.com/540042
    "leak:swrast_dri.so\n"

    // Leak in glibc's gconv caused by fopen(..., "r,ccs=UNICODE")
    "leak:__gconv_lookup_cache\n"

    // ================ Leaks in Chromium code ================
    // PLEASE DO NOT ADD SUPPRESSIONS FOR NEW LEAKS.
    // Instead, commits that introduce memory leaks should be reverted.
    // Suppressing the leak is acceptable in some cases when reverting is
    // impossible, i.e. when enabling leak detection for the first time for a
    // test target with pre-existing leaks.

    // Small test-only leak in ppapi_unittests. http://crbug.com/258113
    "leak:ppapi::proxy::PPP_Instance_Private_ProxyTest_PPPInstancePrivate_"
    "Test\n"

    // http://crbug.com/322671
    "leak:content::SpeechRecognitionBrowserTest::SetUpOnMainThread\n"

    // http://crbug.com/355641
    "leak:TrayAccessibilityTest\n"

    // http://crbug.com/354644
    "leak:CertificateViewerUITest::ShowModalCertificateViewer\n"

    // http://crbug.com/356306
    "leak:service_manager::SetProcessTitleFromCommandLine\n"

    // https://crbug.com/755670
    "leak:third_party/yasm/\n"

    // v8 leaks caused by weak ref not call
    "leak:blink::DOMWrapperWorld::Create\n"
    "leak:blink::ScriptState::Create\n"

    // https://crbug.com/795148
    "leak:third_party/fontconfig/\n"

    // PLEASE READ ABOVE BEFORE ADDING NEW SUPPRESSIONS.

    // End of suppressions.
    ;  // Please keep this semicolon.

#endif  // LEAK_SANITIZER
