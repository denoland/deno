function assert(condition, message) {
  if (!condition) throw new Error(message);
}

Deno.test("jsdom environment globals", () => {
  assert(window === globalThis, "window should alias globalThis");
  assert(
    document.mockDomLibrary === "jsdom",
    "document should come from jsdom",
  );
  assert(
    document.version === "29.1.1",
    "expected the default jsdom version, got " + document.version,
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
    Event.mockDomLibrary === "jsdom",
    "Event should be overridden by the DOM library",
  );
  assert(typeof jsdom === "object", "the JSDOM instance should be exposed");
  assert(jsdom.window.document === document, "document should match");
  assert(fetch !== jsdom.window.fetch, "fetch should stay Deno's");
});
