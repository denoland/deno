addEventListener("foo", () => {
  queueMicrotask(() => console.log("queueMicrotask"));
  setTimeout(() => console.log("timer"), 0);
  throw new Error("bar");
});
console.log(1);
// @ts-ignore Deno.core
Deno.core.opSync("op_set_next_tick_callback", () => console.log("nextTick"));
// @ts-ignore Deno.core
Deno.core.opSync("op_set_has_tick_scheduled", true);
dispatchEvent(new CustomEvent("foo"));
console.log(2);
