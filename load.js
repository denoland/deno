
const code;

const compiled = await WebAssembly.compile(code, {
  "./x.js": x,
});

const imports = WebAssembly.Module.imports(compiled);
const exports = WebAssembly.Module.exports(compiled);

for import in imports {
  import x from import;
}

const instance = new WebAssembly.Instance(compiled, imports);

instance.exports