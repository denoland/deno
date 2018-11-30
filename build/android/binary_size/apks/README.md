## Milestone Reference APKs

This folder contains APKs for official (upstream) builds for each milestone.
The primary use for these APKs is per-milestone binary size analysis.
  * `//build/android/resource_sizes.py` uses them for calculating patch size
  * They can be used with `tools/binary_size/diagnose_bloat.py` for analyzing
    what grew in an APK milestone-to-milestone

## Downloading Reference APKs

```bash
# Downloads ARM 32 MonochromePublic.apk for the latest milestone that we've
# uploaded APKs for.
build/android/binary_size/apk_downloader.py

# Print usage and see all options.
build/android/binary_size/apk_downloader.py -h
```

## Updating Reference APKs
```bash
# Downloads build products from perf builders and uploads the following APKs
# for M62 and M63:
#   ARM 32 - ChromePublic.apk, ChromeModernPublic.apk, MonochromePublic.apk
#   ARM 64 - ChromePublic.apk ChromeModernPublic.apk
build/android/binary_size/apk_downloader.py --update 63 508578 --update 62 499187
```

  * **Remember to commit the generated .sha1 files, update the
    CURRENT_MILESTONE variable in apk_downloader.py, and update the list of
    revisions below**

## Chromium revisions for each APK
  * [M56](https://crrev.com/433059)
  * [M57](https://crrev.com/444943)
  * [M58](https://crrev.com/454471)
  * [M59](https://crrev.com/464641)
  * [M60](https://crrev.com/474934)
  * [M61](https://crrev.com/488528)
  * [M62](https://crrev.com/499187)
  * [M63](https://crrev.com/508578)
  * [M64](https://crrev.com/520840)
  * [M65](https://crrev.com/530369)
  * [M66](https://crrev.com/540276)
  * [M67](https://crrev.com/550428)
