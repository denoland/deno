## Release Schedule

A new minor release for the `deno` cli is released every 6 weeks. A new patch
version is released weekly, except in the week before a new minor release.

The release dates for the upcoming minor releases are:

- 1.8.0: March 2nd, 2021
- 1.9.0: April 13, 2021

Stable releases can be found on the
[GitHub releases page](https://github.com/denoland/deno/releases).

### Canary channel

In addition to the stable channel described above, canaries are released
multiple times daily (for each commit on master). You can upgrade to the latest
canary release by running:

```
deno upgrade --canary
```

To update to a specific canary, pass the commit hash in the `--version` option:

```
deno upgrade --canary --version=973af61d8bb03c1709f61e456581d58386ed4952
```

To switch back to the stable channel, run `deno upgrade`.

Canaries can be downloaded from https://dl.deno.land.
