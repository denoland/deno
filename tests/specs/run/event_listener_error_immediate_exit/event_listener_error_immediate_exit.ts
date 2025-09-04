addEventListener("foo", () => {
  queueMicrotask(() => console.log("queueMicrotask"));
  setTimeout(() => console.log("timer"), 0);
  throw new Error("bar");
});
console.log(1);
// @ts-ignore Deno[Deno.internal].core
Deno[Deno.internal].core.setNextTickCallback(() => console.log("nextTick"));
// @ts-ignore Deno[Deno.internal].core
Deno[Deno.internal].core.setHasTickScheduled(true);
dispatchEvent(new CustomEvent("foo"));
console.log(2);
