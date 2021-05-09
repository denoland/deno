// See issue for details
// https://github.com/denoland/deno/issues/4080
//
// After first received message, this worker schedules
// [assert(), close(), assert()] ops on the same turn of microtask queue
// All tasks after close should not make it

onmessage = async function () {
  let stage = 0;
  await new Promise((_) => {
    setTimeout(() => {
      if (stage !== 0) throw "Unexpected stage";
      stage = 1;
    }, 50);
    setTimeout(() => {
      if (stage !== 1) throw "Unexpected stage";
      stage = 2;
      postMessage("DONE");
      close();
    }, 50);
    setTimeout(() => {
      throw "This should not be run";
    }, 50);
  });
};
