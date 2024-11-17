import { monitorEventLoopDelay } from "node:perf_hooks";

const ht = monitorEventLoopDelay();
ht.enable();
setInterval(function () {
  console.log(ht.min);
  console.log(ht.max);
  console.log(ht.mean);
  console.log(ht.stddev);
  console.log(ht.percentile(50));
  console.log(ht.percentile(99));
  console.log();
}, 1000);

setInterval(function () {
  for (let i = 0; i < 1e7; i++) {
    // simulate event loop blocking
  }
}, 3000);
