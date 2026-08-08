// This package's packument points `dist.tarball` at `registry.npmjs.org` even
// though it is served by the test registry, to exercise relocating the tarball
// download to the configured registry.
module.exports = () => "hi from tarballs-npmjs";
