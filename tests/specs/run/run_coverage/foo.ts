function foo(a: number, b: number) {
  if (a > 0) {
    console.log(a);
  } else {
    console.log(a);
  }

  if (b > 0) {
    console.log(b);
  } else {
    console.log(b);
  }
}

const [a = 0, b = 0] = Deno.args;

foo(+a, +b);
