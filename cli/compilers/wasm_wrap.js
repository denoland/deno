function base64ToUint8Array(data) {{
  const binString = window.atob(data);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {{
    bytes[i] = binString.charCodeAt(i);
  }}
  return bytes;
}}

const buffer = base64ToUint8Array("{}");
const compiled = await WebAssembly.compile(buffer);

const imports = new Set(
  WebAssembly.Module.imports(compiled).map(m => m.module)
);

const importObject = Object.create(null);
for (const module of imports) {{
  importObject[module] = await import(module);
}}

const instance = new WebAssembly.Instance(compiled, importObject);

export default instance.exports;
