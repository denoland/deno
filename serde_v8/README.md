# serde_v8

Serde support for (rusty_)v8

**WIP:** see [denoland/deno#9540](https://github.com/denoland/deno/issues/9540)

## TODO

- [ ] Experiment with KeyCache to optimize struct keys
- [ ] Experiment with external v8 strings
- [ ] Explore using
  [json-stringifier.cc](https://chromium.googlesource.com/v8/v8/+/refs/heads/master/src/json/json-stringifier.cc)'s
  fast-paths for arrays
- [ ] Improve tests to test parity with `serde_json` (should be mostly
  interchangeable)
- [ ] Consider a `Payload` type that's deserializable by itself (holds scope &
  value)
- [ ] Ensure we return errors instead of panicking on `.unwrap()`s
