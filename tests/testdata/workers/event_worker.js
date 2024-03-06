onmessage = function (e) {
  if (e.data === "boom") {
    throw new Error("boom error!");
  }

  postMessage(e.data);
};
