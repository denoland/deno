// Mock OpenAI-like streaming chat completions endpoint
const encoder = new TextEncoder();

const mockServer = Deno.serve({ port: 0, onListen: doFetch }, (req) => {
  const url = new URL(req.url);
  if (url.pathname === "/v1/chat/completions") {
    const stream = new ReadableStream({
      start(controller) {
        controller.enqueue(
          encoder.encode(
            'data: {"id":"chatcmpl-stream1","model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}\n\n',
          ),
        );
        controller.enqueue(
          encoder.encode(
            'data: {"id":"chatcmpl-stream1","model":"gpt-4","choices":[{"index":0,"delta":{"content":"lo!"},"finish_reason":null}]}\n\n',
          ),
        );
        controller.enqueue(
          encoder.encode(
            'data: {"id":"chatcmpl-stream1","model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":8,"completion_tokens":3,"total_tokens":11}}\n\n',
          ),
        );
        controller.enqueue(encoder.encode("data: [DONE]\n\n"));
        controller.close();
      },
    });
    return new Response(stream, {
      headers: { "content-type": "text/event-stream" },
    });
  }
  return new Response("Not found", { status: 404 });
});

async function doFetch({ port }: { port: number }) {
  const resp = await fetch(
    `http://localhost:${port}/v1/chat/completions`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model: "gpt-4",
        messages: [{ role: "user", content: "Hi" }],
        stream: true,
      }),
    },
  );
  // Consume the stream
  const reader = resp.body!.getReader();
  while (true) {
    const { done } = await reader.read();
    if (done) break;
  }
  await mockServer.shutdown();
}
