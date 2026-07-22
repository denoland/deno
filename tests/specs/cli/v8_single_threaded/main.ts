const tids = Array.from(
  Deno.readDirSync("/proc/self/task"),
  ({ name }) => Number(name),
);
const names = tids.map((tid) =>
  Deno.readTextFileSync(`/proc/self/task/${tid}/status`).match(
    /Name:\t(.*)/,
  )![1]
);
console.log(names.some((name) => name.startsWith("V8")));
