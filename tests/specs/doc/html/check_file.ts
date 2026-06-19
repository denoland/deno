const content = Deno.readTextFileSync("./docs/index.html");

if (content.includes("..")) {
  throw new Error("index.html should not link through parent directories");
}

const searchJs = Deno.readTextFileSync("./docs/search.js");

let renderedSearchResults = "";
const searchInput = {
  value: "",
  placeholder: "",
  removeAttribute() {},
  addEventListener() {},
  focus() {},
};
const contentDiv = { style: {} };
const currentFileMeta = {
  attributes: {
    content: {
      value: ".",
    },
  },
};
const searchResultsDiv = {
  style: {},
  set innerHTML(value: string) {
    renderedSearchResults = value;
  },
  get innerHTML() {
    return renderedSearchResults;
  },
};

(globalThis as typeof globalThis & { window: unknown }).window = {
  Fuse: class {
    constructor(private nodes: unknown[]) {}
    search() {
      return this.nodes.map((item) => ({ item }));
    }
  },
  DENO_DOC_SEARCH_INDEX: {
    nodes: [
      { file: ".", name: "MyClass", kind: [] },
      { file: "foo", name: "Nested", kind: [] },
    ],
  },
  location: {
    href: new URL("./docs/~/MyInterface.html?q=My", import.meta.url).href,
  },
  history: {
    replaceState() {},
  },
  addEventListener() {},
};

(globalThis as typeof globalThis & { document: unknown }).document = {
  currentScript: {
    src: new URL("./docs/search.js", import.meta.url).href,
  },
  querySelector(selector: string) {
    if (selector === "#searchbar") {
      return searchInput;
    }
    if (selector === "#content") {
      return contentDiv;
    }
    if (selector === "#searchResults") {
      return searchResultsDiv;
    }
    if (selector === "meta[name='doc-current-file']") {
      return currentFileMeta;
    }
    throw new Error(`Unexpected selector: ${selector}`);
  },
  addEventListener() {},
};

new Function(searchJs)();

const expectedHref = new URL("./docs/./~/MyClass.html", import.meta.url).href;
if (!renderedSearchResults.includes(`href="${expectedHref}"`)) {
  throw new Error(
    `Search result href should resolve from the docs root. Expected ${expectedHref} in ${renderedSearchResults}`,
  );
}

const expectedNestedHref = new URL(
  "./docs/foo/~/Nested.html",
  import.meta.url,
).href;
if (!renderedSearchResults.includes(`href="${expectedNestedHref}"`)) {
  throw new Error(
    `Nested search result href should resolve from the docs root. Expected ${expectedNestedHref} in ${renderedSearchResults}`,
  );
}
