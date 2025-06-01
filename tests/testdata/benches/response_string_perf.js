const mixed = "@Ā๐😀";

function generateRandom(bytes) {
  let result = "";
  let i = 0;
  while (i < bytes) {
    const toAdd = Math.floor(Math.random() * Math.min(4, bytes - i));
    switch (toAdd) {
      case 0:
        result += mixed[0];
        i++;
        break;
      case 1:
        result += mixed[1];
        i++;
        break;
      case 2:
        result += mixed[2];
        i++;
        break;
      case 3:
        result += mixed[3];
        result += mixed[4];
        i += 2;
        break;
    }
  }
  return result;
}

const randomData = generateRandom(1024);
for (let i = 0; i < 10_000; i++) {
  new Response(randomData);
}
