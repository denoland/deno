eval("");

for (let i = 0; i < 10; i++) {
  await Deno.open("test");
}

const unused = 1;

export function test(): any {
  return {};
}
