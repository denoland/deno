const importObject = Object.create(null);
//IMPORTS

function base64ToUint8Array(data) {
  const binString = window.atob(data);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    bytes[i] = binString.charCodeAt(i);
  }
  return bytes;
}

const buffer = base64ToUint8Array("BASE64_DATA");
const compiled = await WebAssembly.compile(buffer);

const instance = new WebAssembly.Instance(compiled, importObject);

//EXPORTS
