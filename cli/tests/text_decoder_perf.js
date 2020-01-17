const mixed = new TextEncoder().encode("@Ā๐😀");

function generateRandom(bytes) {
  const result = new Uint8Array(bytes);
  let i = 0;
  while (i < bytes) {
    const toAdd = Math.floor(Math.random() * Math.min(4, bytes - i));
    switch (toAdd) {
      case 0:
        result[i] = mixed[0];
        i++;
        break;
      case 1:
        result[i] = mixed[1];
        result[i + 1] = mixed[2];
        i += 2;
        break;
      case 2:
        result[i] = mixed[3];
        result[i + 1] = mixed[4];
        result[i + 2] = mixed[5];
        i += 3;
        break;
      case 3:
        result[i] = mixed[6];
        result[i + 1] = mixed[7];
        result[i + 2] = mixed[8];
        result[i + 3] = mixed[9];
        i += 4;
        break;
    }
  }
  return result;
}

const randomData = generateRandom(1024);
const decoder = new TextDecoder();
for (let i = 0; i < 10_000; i++) decoder.decode(randomData);
