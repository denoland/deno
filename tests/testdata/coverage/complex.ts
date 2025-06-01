/**
 * @module
 * Complex module
 */

// This entire interface should be completely ignored by the coverage tool.
export interface Complex {
  // These comments should be ignored.
  foo: string;

  // But this is a stub, so this isn't really documentation.
  bar: string;

  // Really all these are doing is padding the line count.
  baz: string;
}

// Lets add some wide characters to ensure that the absolute byte offsets are
// being matched properly.
//
// 패딩에 대한 더 많은 문자.
function dependency(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  // inline comment in tested function
  return {
    foo,
    bar,
    baz,
  };
}

// Again just more wide characters for padding.
//
// 良い対策のためにいくつかのユニコード文字を投げる。
export function complex(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  /*
   * block comment in tested function
   */
  return dependency(
    foo,
    bar,
    baz,
  );
}

// And yet again for good measure.
// 更多用於填充的字元。
export function unused(
  foo: string,
  bar: string,
  baz: string,
): Complex {
  // inline comment in untested function
  return complex(
    foo,
    bar,
    baz,
  );
}

// Using a non-ascii name again to ensure that the byte offsets match up
// correctly.
export const π = Math.PI;

// And same applies for this one, this one is unused and will show up in
// lacking coverage.
export function ƒ(): number {
  /*
   * block comment in untested function
   */
  return (
    0
  );
}

// This arrow function should also show up as uncovered.
console.log("%s", () => 1);

// Make sure emojis work properly
console.log("📣❓");
