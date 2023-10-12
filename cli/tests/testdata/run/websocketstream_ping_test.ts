const wss = new WebSocketStream("ws://127.0.0.1:4513");
const { readable } = await wss.opened;
for await (const _ of readable) {
  //
}
