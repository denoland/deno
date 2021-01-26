// This entire interface should be completely ignored by the coverage tool.
export interface Complex {
  // These are comments.
  foo: string;

  // But this is a stub, so this isn't really documentation.
  bar: string;

  // Really all these are doing is padding the line count.
  baz: string;
}

export function complex(
  foo: string,
  bar: string,
  baz: string
): Complex {
  return {
    foo,
    bar,
    baz
  };
}

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
