// const ws = new WebSocket("ws://localhost:4242");
// ws.onopen = function () {
//   ws.send("Hello");
//   ws.send(new Uint8Array([1, 2, 3]));
// };
// ws.onmessage = function (event) {
//   console.log(event.data);
// };

// setTimeout(() => ws.close(1000, "foo"), 1000);

const resp = await fetch("https://deno.land/std/version.ts");
await resp.text();
