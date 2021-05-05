## Release Schedule

A new minor release for the `deno` cli is released every 6 weeks. After 1.9.0 we
will be switching to a 4 week release cycle. A new patch version is released
weekly, as necessary.

The release dates for the upcoming minor releases are:

- 1.9.0: April 13, 2021
- 1.10.0: May 11, 2021
- 1.11.0: June 8, 2021

Stable releases can be found on the
[GitHub releases page](https://github.com/denoland/deno/releases).

### Canary channel

In addition to the stable channel described above, canaries are released
multiple times daily (for each commit on main). You can upgrade to the latest
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
