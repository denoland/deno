function assert(condition, message) {
  if (!condition) throw new Error(message);
}

Deno.test("dom environment globals", () => {
  const domWindow = getWindow();
  assert(window === globalThis, "window should alias globalThis");
  assert(self === globalThis, "self should alias globalThis");
  assert(top === globalThis, "top should alias globalThis");
  assert(parent === globalThis, "parent should alias globalThis");
  assert(
    document.mockDomLibrary === "happy-dom",
    "document should come from happy-dom",
  );
  assert(
    document.version === "20.10.2",
    "expected the default happy-dom version, got " + document.version,
  );
  assert(
    document.defaultView === globalThis,
    "document.defaultView should be globalThis",
  );
  assert(
    location.href === "http://localhost:3000/",
    "unexpected location: " + location.href,
  );
  assert(
    Event.mockDomLibrary === "happy-dom",
    "Event should be overridden by the DOM library",
  );
  assert(
    new CustomEvent("x") instanceof Event,
    "CustomEvent should subclass the DOM library Event",
  );
  assert(
    EventTarget.mockDomLibrary === "happy-dom",
    "EventTarget should be overridden by the DOM library",
  );
  assert(
    FormData.mockDomLibrary === "happy-dom",
    "FormData should be overridden by the DOM library",
  );
  assert(fetch !== domWindow.fetch, "fetch should stay Deno's");
  assert(
    happyDOM.version === "20.10.2",
    "happyDOM helper should be exposed",
  );
  const el = document.createElement("div");
  assert(el.tagName === "DIV", "createElement should work");
});
