let value = 0;
async function counter(max: number) {
  value += 1;

  await new Promise((resolve) => {
    setTimeout(function recheck() {
      setTimeout(value == max ? resolve : recheck, 0);
    }, 0);
  });
}

const count = 10;
for (let i = 0; i < count; i++) {
  Deno.test({
    name: `counter ${i + 1}`,
    async fn() {
      return await counter(count);
    },
    concurrent: true,
  });
}
