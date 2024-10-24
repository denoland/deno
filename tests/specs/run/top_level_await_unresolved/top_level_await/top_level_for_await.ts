async function* asyncGenerator(): AsyncIterableIterator<number> {
  let i = 0;
  while (i < 3) {
    yield i++;
  }
}

for await (const num of asyncGenerator()) {
  console.log(num);
}
