// Regression test for https://github.com/denoland/deno/issues/22592
// deno-lint-ignore require-await
async function rejects() {
  return Promise.reject();
}
await rejects();
