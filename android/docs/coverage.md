# Android code coverage instructions

These are instructions for collecting code coverage data for android
instrumentation and junit tests.

[TOC]

## How EMMA coverage works

In order to use EMMA code coverage, we need to create build time **.em** files
and runtime **.ec** files. Then we need to process them using the
build/android/generate_emma_html.py script.

## How to collect EMMA coverage data

1. Use the following GN build arguments:

```
    target_os = "android"
    emma_coverage = true
    emma_filter = "org.chromium.chrome.browser.ntp.*,-*Test*,-*Fake*,-*Mock*"
```

The filter syntax is as documented for the [EMMA coverage
filters](http://emma.sourceforge.net/reference/ch02s06s02.html).

Now when building, **.em** files will be created in the build directory.

2. Run tests, with option `--coverage-dir <directory>`, to specify where to save
   the .ec file. For example, you can run chrome junit tests:
   `out/Debug/bin/run_chrome_junit_tests --coverage-dir /tmp/coverage`.

3. Turn off strict mode when running instrumentation tests by adding
   `--strict-mode=off` because the EMMA code causes strict mode violations by
   accessing disk.

4. Use a pre-L Android OS (running Dalvik) because code coverage is not
   supported in ART.

5. The coverage results of junit and instrumentation tests will be merged
   automatically if they are in the same directory.

6. Now we have both .em and .ec files. We can create a html report using
   `generate_emma_html.py`, for example:

```
   build/android/generate_emma_html.py \
       --coverage-dir /tmp/coverage/ \
       --metadata-dir out/Debug/ \
       --output example.html
```
   Then an example.html containing coverage info will be created:

```
   EMMA: writing [html] report to [<your_current_directory>/example.html] ...
```
