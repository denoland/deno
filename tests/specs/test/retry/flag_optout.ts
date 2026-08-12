// An explicit `retry: 0` opts out of the `--retry` flag default and takes
// precedence over it. With `--retry=5` this always-failing test still runs
// only once and FAILS with no retry attempts (otherwise there would be five
// "retrying" lines).
Deno.test({
  name: "optout",
  retry: 0,
  fn() {
    throw new Error("always");
  },
});
