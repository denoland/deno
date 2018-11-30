# java_deobfuscate

A wrapper around ProGuard's ReTrace tool, which:

1) Updates the regular expression used to identify stack lines, and
2) Streams its output.

The second point here is what allows you to run:

    adb logcat | out/Default/bin/java_deobfuscate out/Default/apks/ChromePublic.apk.mapping

And have it actually show output without logcat terminating.


# stackwalker.py

Extracts Breakpad microdumps from a log file and uses `stackwalker` to symbolize
them.
