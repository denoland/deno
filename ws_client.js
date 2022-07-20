const ws = new WebSocket("ws://localhost:9000/");
ws.onopen = function () {
  console.log("open");
  ws.send("hello");
};
ws.onmessage = function (e) {
  console.log(e.data);
};
ws.onclose = function () {
  console.log("close");
};
ws.onerror = function (e) {
  console.log("error", e);
};
