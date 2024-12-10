"foo\n";
'bar"';
2;
2.3;
0b000;
true;
false;
null;
/a/g;
/[/g]/m;
1n;

// arrays
[1, 2, 3];
[1, ...foo];
[1, , 3];

// objects
a = {};
a = { foo };
a = { foo: 1 };
a = { ...foo };
a = { ["foo\n"]: 1, 1: 2, "baz": 3 };
a = {
  get foo() {
    return 1;
  },
  // FIXME
  // set foo(a) {
  //   2;
  // },
  // bar() {},
  // async barAsync() {},
  // *barGen() {},
  // async *barAsyncGen() {},
};

a = `foo`;
a = `foo${" "}bar`;
