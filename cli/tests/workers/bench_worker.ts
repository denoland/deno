onmessage = function (e): void {
  const { cmdId, action, data } = e.data;
  switch (action) {
    case 0: // Static response
      postMessage({
        cmdId,
        data: "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n",
      });
      break;
    case 1: // Respond with request data
      postMessage({ cmdId, data });
      break;
    case 2: // Ping
      postMessage({ cmdId });
      break;
    case 3: // Close
      postMessage({ cmdId: 3 });
      close();
      break;
  }
};
