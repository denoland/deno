// Copyright 2018-2025 the Deno authors. MIT license.
import { assertStrictEquals } from "./test_util.ts";

Deno.test(
  { permissions: { net: ["127.0.0.1"] } },
  async function eventSourceColonInMessage() {
    const portDeferred = Promise.withResolvers<number>();

    await using _server = Deno.serve({
      handler: () =>
        new Response('data: {"key":"value"}\n\n', {
          headers: { "content-type": "text/event-stream" },
        }),
      onListen: ({ port }) => portDeferred.resolve(port),
      hostname: "127.0.0.1",
      port: 0,
    });

    const port = await portDeferred.promise;
    const eventSource = new EventSource(`http://127.0.0.1:${port}/`);
    const event = await new Promise<MessageEvent>((resolve) =>
      eventSource.onmessage = resolve
    );
    eventSource.close();
    assertStrictEquals(event.data, '{"key":"value"}');
  },
);
