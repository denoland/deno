// deno-lint-ignore-file
let x = {
  a: {
    b: {
      c() {
        return { d: "hello" };
      },
    },
  },
};
let y = {
  a: {
    b: {
      c() {
        return { d: 1234 };
      },
    },
  },
};
x = y;
