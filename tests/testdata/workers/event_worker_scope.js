let messageHandlersCalled = 0;
let errorHandlersCalled = 0;

onmessage = function (e) {
  if (e.data === "boom") {
    throw new Error("boom error!");
  }
  messageHandlersCalled++;
};

self.addEventListener("message", (_e) => {
  messageHandlersCalled++;
});

self.addEventListener("message", (_e) => {
  messageHandlersCalled++;
});

self.addEventListener("message", (_e) => {
  messageHandlersCalled++;

  postMessage({
    messageHandlersCalled,
    errorHandlersCalled,
  });
});

onerror = function (_e) {
  errorHandlersCalled++;
};

self.addEventListener("error", (_e) => {
  errorHandlersCalled++;
});

self.addEventListener("error", (_e) => {
  errorHandlersCalled++;
});

self.addEventListener("error", (e) => {
  errorHandlersCalled++;
  e.preventDefault();
});
