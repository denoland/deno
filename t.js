import { setImmediate } from "node:timers";

let ticks = 0;
const startTime = performance.now();
const loop = () => {
  ticks++
  if (performance.now() - startTime > 1000) {
    console.log(ticks, "ticks per second");
    return;
  }
  setImmediate(loop);
}
loop();
