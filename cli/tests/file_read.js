const minPowTwo = 2;
const maxPowTwo = 1024 * 1024;

const END_MARKER = "b";
const POW_TWO_MARKER = "m";
const CHAR_MARKER = "a";

const logIfNotProperEntry = (n, len) => {
  const end = String.fromCharCode(n[len - 1]);
  if (end !== END_MARKER) {
    console.log(
      `String for file input size ${len} not properly read, invalid char at position ${n
        .length - 1}, ${END_MARKER} !== ${end}`,
    );
    return;
  }

  for (let k = 0; k < len - 1; ++k) {
    const charAtKPos = String.fromCharCode(n[k]);
    if (k > 1 && (k & (k - 1)) === 0) {
      if (charAtKPos !== POW_TWO_MARKER) {
        console.log(
          `String for file input size ${len} not properly read, invalid char at position ${k}, ${POW_TWO_MARKER} !== ${charAtKPos}`,
        );
        return;
      }
    } else if (charAtKPos !== CHAR_MARKER) {
      console.log(
        `String for file input size ${len} not properly read, invalid char at position ${k}, ${CHAR_MARKER} !== ${charAtKPos}`,
      );
      return;
    }
  }
};

const textWithBufferSize = async (file, bufferSize, maxSize) => {
  console.log(`Case bufferSize: ${bufferSize}, maxSize: ${maxSize}`);
  await Deno.seek(file.rid, 0, Deno.SeekMode.Start);

  let finalBuffer = null;
  let bytesRead = 0;
  while (bytesRead < maxSize) {
    const slice = new Uint8Array(bufferSize);
    const read = await Deno.read(file.rid, slice);
    if (read === null) {
      break;
    }

    bytesRead += read;
    if (finalBuffer === null) {
      finalBuffer = slice;
    } else {
      const len = finalBuffer.length;
      const subslice = slice.subarray(0, read);
      const buffer = new Uint8Array(len + read);
      buffer.set(finalBuffer, 0);
      buffer.set(subslice, len);
      finalBuffer = buffer;
    }
  }

  const textBuffer = finalBuffer.subarray(0, bytesRead);
  const text = decoder.decode(textBuffer);
  console.log(`READ ${bytesRead}`);
  console.log(text[bytesRead - 1]);
  logIfNotProperEntry(textBuffer, bytesRead);
  console.log(bytesRead);
  console.log(text.length);
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();
for (let i = minPowTwo; i <= maxPowTwo; i *= 2) {
  const fileName = await Deno.makeTempFile();
  const file = await Deno.open(fileName, { read: true, write: true });
  let data = "";
  for (let j = 0; j < i - 1; ++j) {
    if (j > 1 && (j & (j - 1)) === 0) {
      data += POW_TWO_MARKER;
    } else {
      data += CHAR_MARKER;
    }
  }

  data += END_MARKER;

  const input = encoder.encode(data);
  await Deno.write(file.rid, input);
  await textWithBufferSize(file, i, i);
  await textWithBufferSize(file, 2 * i, i);
  await textWithBufferSize(file, 300, i);
  await textWithBufferSize(file, 1024 * 16, i);
  await textWithBufferSize(file, (1024 * 16) - 1, i);
  await textWithBufferSize(file, 1024 * 32, i);

  await Deno.remove(fileName);
  await Deno.close(file.rid);
}
