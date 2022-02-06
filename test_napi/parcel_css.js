const lib = Deno.core.dlopen(
  "./node_modules/@parcel/css-darwin-arm64/parcel-css.darwin-arm64.node",
);

// Test case from @parcel/css
// https://github.com/parcel-bundler/parcel-css/blob/1e89b39cd922d2e577c8e39611f484a525fd8937/test.js
const result = lib.transform({
  filename: "test.css",
  minify: false,
  targets: {
    safari: 4 << 16,
    firefox: 3 << 16 | 5 << 8,
    opera: 10 << 16 | 5 << 8,
  },
  code: Deno.core.encode(`
  @import "foo.css";
  @import "bar.css" print;
  @import "baz.css" supports(display: grid);
  .foo {
      composes: bar;
      composes: baz from "baz.css";
      color: pink;
  }
  .bar {
      color: red;
      background: url(test.jpg);
  }
  `),
  drafts: {
    nesting: true,
  },
  cssModules: true,
  analyzeDependencies: true,
});

console.log(Deno.core.decode(result.code));
console.log(JSON.stringify(result.exports));
console.log(JSON.stringify(result.dependencies));
