# Lint

Android's [**lint**](http://developer.android.com/tools/help/lint.html) is a static
analysis tool that Chromium uses to catch possible issues in Java code.

[TOC]

## How Chromium uses lint

Chromium runs lint on a per-target basis for all targets using any of the
following templates if they are marked as Chromium code (i.e.,
`chromium_code = true`):

 - `android_apk`
 - `android_library`
 - `instrumentation_test_apk`
 - `unittest_apk`

Chromium also runs lint on a per-target basis for all targets using any of the
following templates if they are marked as Chromium code and they support
Android (i.e., `supports_android = true`): 

 - `java_library`

This is implemented in the
[`android_lint`](https://code.google.com/p/chromium/codesearch#chromium/src/build/config/android/internal_rules.gni&q=android_lint%20file:internal_rules%5C.gni)
gn template.

## My code has a lint error

If lint reports an issue in your code, there are several possible remedies.
In descending order of preference:

### Fix it

While this isn't always the right response, fixing the lint error or warning
should be the default.

### Suppress it in code

Android provides an annotation,
[`@SuppressLint`](http://developer.android.com/reference/android/annotation/SuppressLint.html),
that tells lint to ignore the annotated element. It can be used on classes,
constructors, methods, parameters, fields, or local variables, though usage
in Chromium is typically limited to the first three.

Like many suppression annotations, `@SuppressLint` takes a value that tells **lint**
what to ignore. It can be a single `String`:

```java
@SuppressLint("NewApi")
public void foo() {
    a.methodThatRequiresHighSdkLevel();
}
```

It can also be a list of `String`s:

```java
@SuppressLint({
        "NewApi",
        "UseSparseArrays"
        })
public Map<Integer, FakeObject> bar() {
    Map<Integer, FakeObject> shouldBeASparseArray = new HashMap<Integer, FakeObject>();
    another.methodThatRequiresHighSdkLevel(shouldBeASparseArray);
    return shouldBeASparseArray;
}
```

This is the preferred way of suppressing warnings in a limited scope.

### Suppress it in the suppressions XML file

**lint** can be given an XML configuration containing warnings or errors that
should be ignored. Chromium's lint suppression XML file can be found in
[`build/android/lint/suppressions.xml`](https://chromium.googlesource.com/chromium/src/+/master/build/android/lint/suppressions.xml).
It can be updated to suppress current warnings by running:

```bash
$ python build/android/lint/suppress.py <result.xml file>
```

e.g., to suppress lint errors found in `media_java`:

```bash
$ python build/android/lint/suppress.py out/Debug/gen/media/base/android/media_java__lint/result.xml
```

**This mechanism should only be used for disabling warnings across the entire code base; class-specific lint warnings should be disabled inline.**

