// Plain Deno code that should always type-check cleanly. If the extends
// filter regresses, the generated `.deno/tsconfig.json` will leak into
// Deno's checker and produce spurious errors here.
const _dir: string | undefined = import.meta.dirname;
const _url: string = import.meta.url;
console.log(_dir, _url);
