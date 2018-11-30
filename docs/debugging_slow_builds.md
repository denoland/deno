# Debugging slow builds

Some tips for debugging slow build times:
* Use [ninjatracing](https://github.com/nico/ninjatracing) and chrome:tracing to
  view a timeline of the most recent build.
  * Many bots output a build trace (look for a `"ninja_log"` link).
* Use `gn gen --tracelog trace.json` to create a similar trace for `gn gen`.
* Depot Tool's `autoninja` has logic for summarizing slow steps. Enable it via:
  * `NINJA_SUMMARIZE_BUILD=1 autoninja -C out/Debug my_target`
* Many Android templates make use of
  [`md5_check.py`](https://cs.chromium.org/chromium/src/build/android/gyp/util/md5_check.py)
  to optimize incremental builds.
  * Set `PRINT_BUILD_EXPLANATIONS=1` to have these commands log which inputs
    changed.
* If you suspect files are being rebuilt unnecessarily during incremental
  builds:
  * Use `ninja -n -d explain` to figure out why ninja thinks a target is dirty.
  * Ensure actions are taking advantage of ninja's `restat=1` feature by not
    updating timestamps on outputs when their content does not change.
