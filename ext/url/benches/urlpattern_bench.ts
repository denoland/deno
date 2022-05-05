const CASES = {
  "literal": ["/about", "https://example.com/about"],
  "segment": ["/:foo", "https://example.com/bar"],
  "prefixed segment": ["/abc/:foo", "https://example.com/abc/bar"],
  "prefixed + suffixed segment": [
    "/abc/:foo.html",
    "https://example.com/abc/bar.html",
  ],
  "wilcard": ["*", "https://example.com/abc/bar.html"],
  "prefixed wilcard": ["/abc/*", "https://example.com/abc/bar.html"],
  "prefixed + suffixed wilcard": [
    "/abc/*.html",
    "https://example.com/abc/bar.html",
  ],
  "custom regexp": [
    "/abc/:foo(\\d+)",
    "https://example.com/abc/123",
  ],
};

for (const [name, cases] of Object.entries(CASES)) {
  const pattern = new URLPattern({ pathname: cases[0] });
  const url = cases[1];
  Deno.bench(name, () => {
    for (let i = 0; i < 10; i++) {
      pattern.exec(url);
    }
  });
}

const ROUTES = [
  "/",
  "/about",
  "/contact",
  "/blog",
  "/blog/:id",
];

const URL_PATTERNS = ROUTES.map((route) => new URLPattern({ pathname: route }));

const TESTS = [
  "/",
  "/about",
  "/contact",
  "/blog",
  "/blog/1",
  "/blog/2",
  "/blog/3",
];

function urlpattern(path: string) {
  for (let i = 0; i < URL_PATTERNS.length; i++) {
    const match = URL_PATTERNS[i].exec({ pathname: path });
    if (match) return [i, match];
  }
}

Deno.bench("urlpattern", { group: "1" }, () => {
  for (const test of TESTS) {
    urlpattern(test);
  }
});

function handwritten(path: string) {
  if (path === "/") {
    return [0, {}];
  }
  if (path === "/about") {
    return [1, {}];
  }
  if (path === "/contact") {
    return [2, {}];
  }
  if (path === "/blog") {
    return [3, {}];
  }
  if (path.startsWith("/blog")) {
    const match = URL_PATTERNS[4].exec({ pathname: path });
    if (match !== null) return [4, match.pathname.groups];
  }
  return null;
}

Deno.bench("handwritten", { group: "1" }, () => {
  for (const test of TESTS) {
    handwritten(test);
  }
});
