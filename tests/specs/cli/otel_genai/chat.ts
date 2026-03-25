// Mock OpenAI-like chat completions endpoint on localhost
const mockServer = Deno.serve({ port: 0, onListen: doFetch }, (req) => {
  const url = new URL(req.url);
  if (url.pathname === "/v1/chat/completions") {
    return Response.json({
      id: "chatcmpl-test123",
      object: "chat.completion",
      model: "gpt-4",
      choices: [
        {
          index: 0,
          message: { role: "assistant", content: "Hello!" },
          finish_reason: "stop",
        },
      ],
      usage: {
        prompt_tokens: 10,
        completion_tokens: 5,
        total_tokens: 15,
      },
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
        messages: [{ role: "user", content: "Say hello" }],
        temperature: 0.7,
        max_tokens: 100,
      }),
    },
  );
  await resp.text();
  await mockServer.shutdown();
}
