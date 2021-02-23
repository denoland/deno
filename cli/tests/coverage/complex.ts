// This entire interface should be completely ignored by the coverage tool.
export interface Complex {
  // These are comments.
  foo: string;

  // But this is a stub, so this isn't really documentation.
  bar: string;

  // Really all these are doing is padding the line count.
  baz: string;
}

// 패딩에 대한 더 많은 문자.
function dependency(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  return {
    foo,
    bar,
    baz,
  };
}

// 良い対策のためにいくつかのユニコード文字を投げる。
export function complex(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  return dependency(
    foo,
    bar,
    baz,
  );
}

// 更多用於填充的字元。
export function unused(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  return complex(
    foo,
    bar,
    baz,
  );
}
