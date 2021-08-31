self.onmessage = (evt) => {
  console.log("Target from self.onmessage:", String(evt.target));
};

self.addEventListener("message", (evt) => {
  console.log("Target from message event listener:", String(evt.target));

  // Throw an error here so the global's error event will fire.
  throw new Error("Some error message");
});

self.onerror = (...args) => {
  console.log("Arguments from self.onerror:", args);
  return true;
};

self.addEventListener("error", (evt) => {
  // Returning true from self.onerror means that subsequent event listeners
  // should see the event as canceled.
  console.log("Is event canceled?:", evt.defaultPrevented);

  self.close();
});
