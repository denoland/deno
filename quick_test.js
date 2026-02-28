// Quick test to see if our fix works
const server = Deno.serve({ port: 8082 }, (req) => {
  const { socket, response } = Deno.upgradeWebSocket(req);

  socket.onopen = () => {
    console.log("Server: Connection opened");
    // Server automatically closes after short delay
    setTimeout(() => {
      console.log("Server: Sending close with code 4222");
      socket.close(4222, "server initiated");
    }, 500);
  };

  socket.onclose = (event) => {
    console.log(
      `Server: Connection closed. Code: ${event.code}, Reason: ${event.reason}`,
    );
    server.shutdown();
  };

  return response;
});

console.log("Server starting on port 8082");

// Give server time to start
await new Promise((resolve) => setTimeout(resolve, 100));

// Client test
const ws = new WebSocketStream("ws://localhost:8082");
await ws.opened;
console.log("Client: Connection opened");
console.log("WebSocketStream properties:", Object.getOwnPropertyNames(ws));

// Client closes first with its own code before server close arrives
setTimeout(() => {
  console.log("Client: Sending close with code 4111");
  ws.close({ closeCode: 4111, reason: "client initiated" });
}, 200);

// Wait for final close
const closeInfo = await ws.closed;
console.log(
  `Client: Final close info - Code: ${closeInfo.closeCode}, Reason: ${closeInfo.reason}`,
);

if (closeInfo.closeCode === 4222 && closeInfo.reason === "server initiated") {
  console.log("✅ SUCCESS: Remote close code/reason correctly returned");
} else {
  console.log("❌ FAIL: Got wrong close code/reason");
  console.log("Expected: 4222 'server initiated'");
  console.log(`Got: ${closeInfo.closeCode} '${closeInfo.reason}'`);
}
