// Test script to reproduce the remote-close issue

const server = Deno.serve({ port: 8081 }, (req) => {
  const url = new URL(req.url);
  if (url.pathname === "/remote-close") {
    const code = url.searchParams.get("code") || "1005";
    const reason = url.searchParams.get("reason") || "";

    if (req.headers.get("upgrade") === "websocket") {
      const { response, socket } = Deno.upgradeWebSocket(req);

      socket.onopen = () => {
        console.log("Server: WebSocket opened");
        // Send close frame with remote code/reason after a short delay
        setTimeout(() => {
          console.log(
            `Server: Sending close frame with code=${code}, reason="${reason}"`,
          );
          socket.close(parseInt(code), reason);
        }, 10);
      };

      socket.onclose = (event) => {
        console.log(
          `Server: WebSocket closed with code=${event.code}, reason="${event.reason}"`,
        );
      };

      return response;
    }
  }

  return new Response("Not found", { status: 404 });
});

// Test the remote close behavior
async function testRemoteClose() {
  console.log("\n=== Testing remote close behavior ===");

  try {
    const wss = new globalThis.WebSocketStream(
      "ws://localhost:8081/remote-close?code=4222&reason=remote",
    );
    console.log("Client: Opening WebSocket...");

    await wss.opened;
    console.log("Client: WebSocket opened");

    console.log(
      'Client: Sending local close frame with code=4111, reason="local"',
    );
    wss.close({ closeCode: 4111, reason: "local" });

    const { closeCode, reason } = await wss.closed;
    console.log(
      `Client: WebSocket closed with code=${closeCode}, reason="${reason}"`,
    );

    if (closeCode === 4222 && reason === "remote") {
      console.log("✅ SUCCESS: Remote code and reason were used correctly!");
    } else {
      console.log(
        '❌ FAILURE: Expected code=4222, reason="remote", but got code=' +
          closeCode + ', reason="' + reason + '"',
      );
    }
  } catch (error) {
    console.error("Test error:", error);
  }

  server.shutdown();
}

// Give server a moment to start, then run test
setTimeout(testRemoteClose, 100);
