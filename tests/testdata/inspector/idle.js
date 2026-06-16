// Keeps the event loop alive but idle: every iteration just awaits a timer, so
// the process spends virtually all of its time parked waiting rather than
// running JS. Used to verify that the CPU profiler attributes this wait time to
// the "(idle)" node instead of "(program)". See
// https://github.com/denoland/deno/issues/21620.
async function main() {
  while (true) {
    await new Promise((resolve) => setTimeout(resolve, 2000));
  }
}

console.log("hello!");
main();
