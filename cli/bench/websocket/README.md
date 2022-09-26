## websocket

```
# Start server
deno run --allow-net --unstable cli/bench/websocket/server.js
```

```
# Deno WebSocket client
deno run --allow-hrtime --allow-net --unstable cli/bench/websocket/client.js

# Node `ws` client
node cli/bench/websocket/client.js

# tokio_tungstenite
target/release/ws_client_perf
```