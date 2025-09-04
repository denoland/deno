// buffer larger than file
{
  using file = Deno.openSync(import.meta.dirname + "/data/1.txt");
  const data = new Uint8Array(13);
  const len = file.readSync(data);
  if (len !== 13) {
    throw new Error("Unexpected read length");
  }
  if (file.readSync(new Uint8Array(1024)) !== null) {
    throw new Error("Unexpected.");
  }
  const textData = new TextDecoder().decode(data);
  if (textData !== "Hello, world!") {
    throw new Error("Unexpected file data (1): " + textData);
  }
}

// buffer smaller than file
{
  using file = Deno.openSync(import.meta.dirname + "/data/1.txt");
  const finalData = new Uint8Array(13);
  const data = new Uint8Array(2);
  let pos = 0;
  while (true) {
    const len = file.readSync(data);
    if (len === 0 || len == null) {
      break;
    }
    finalData.set(data.subarray(0, len), pos);
    pos += len;
  }
  const textData = new TextDecoder().decode(finalData);
  if (textData !== "Hello, world!") {
    throw new Error("Unexpected file data (2): " + textData);
  }
}

// large amount of data, small reads
{
  const bytes = new Uint8Array((1024 ** 2) * 20);
  using file = Deno.openSync(import.meta.dirname + "/data/2.dat");
  const buffer = new Uint8Array(2);
  let pos = 0;
  while (true) {
    const len = file.readSync(buffer);
    if (len === 0 || len == null) {
      break;
    }
    bytes.set(buffer.subarray(0, len), pos);
    pos += len;
  }
  for (let i = 0; i < bytes.length; i++) {
    if (bytes[i] !== i % 256) {
      throw new Error("Unexpected data.");
    }
  }
}
