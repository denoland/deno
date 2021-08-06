// If this is executed with --allow-net but not --allow-read the following
// import should cause a permission denied error.
(async () => {
  await import("http://localhost:4545/cli/tests/subdir/evil_remote_import.js");
})();
