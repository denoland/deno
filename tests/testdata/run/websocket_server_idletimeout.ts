import { assertEquals } from "@std/assert";

Deno.serve({
  port: 4509,
  onListen() {
    console.log("READY");
  },
  handler(request) {
    const { response, socket } = Deno.upgradeWebSocket(request, {
      idleTimeout: 1,
    });
    socket.onerror = (e) => {
      console.log(e);
      assertEquals((e as ErrorEvent).message, "No response from ping frame.");
      // TODO(mmastrac): this doesn't exit on its own. Why?
      Deno.exit(123);
    };
    socket.onclose = (e) => {
      console.log(e);
      assertEquals(e.reason, "No response from ping frame.");
      // TODO(mmastrac): this doesn't exit on its own. Why?
      Deno.exit(123);
    };
    return response;
  },
});
