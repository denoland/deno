The `build` directory contains a forked version of Chromium's 'build'
repository. To merge upstream changes, use the following command:

```sh
$ git subtree pull --squash --prefix build https://chromium.googlesource.com/chromium/src/build.git «SHA»
```
