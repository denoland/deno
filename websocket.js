const ws = new WebSocket("ws://localhost:8080");
ws.onerror = (e) => {
  console.log(e.error);
  console.log(new Error().stack);
};
