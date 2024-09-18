import { assertEquals } from "@std/assert";

const errorDeferred = Promise.withResolvers<void>();
const closeDeferred = Promise.withResolvers<void>();

const listener = Deno.listen({ port: 4509 });
console.log("READY");
// @ts-ignore `Deno.serveHttp()` was soft-removed in Deno 2.
const httpConn = Deno.serveHttp(await listener.accept());
const { request, respondWith } = (await httpConn.nextRequest())!;
const { response, socket } = Deno.upgradeWebSocket(request, {
  idleTimeout: 1,
});
socket.onerror = (e) => {
  console.log(e);
  assertEquals((e as ErrorEvent).message, "No response from ping frame.");
  errorDeferred.resolve();
};
socket.onclose = (e) => {
  console.log(e);
  assertEquals(e.reason, "No response from ping frame.");
  closeDeferred.resolve();
};
await respondWith(response);

await errorDeferred.promise;
await closeDeferred.promise;

// TODO(mmastrac): this doesn't exit on its own. Why?
Deno.exit(123);
