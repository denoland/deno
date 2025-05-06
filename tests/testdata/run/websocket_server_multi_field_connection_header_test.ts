const { promise, resolve } = Promise.withResolvers<void>();
const listener = Deno.listen({ port: 4319 });
console.log("READY");
const conn = await listener.accept();
// @ts-ignore `Deno.serveHttp()` was soft-removed in Deno 2.
const httpConn = Deno.serveHttp(conn);
const { request, respondWith } = (await httpConn.nextRequest())!;
const {
  response,
  socket,
} = Deno.upgradeWebSocket(request);
socket.onerror = () => Deno.exit(1);
socket.onopen = () => socket.close();
socket.onclose = () => resolve();
await respondWith(response);
await promise;
