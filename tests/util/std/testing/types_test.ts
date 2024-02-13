// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright @dsherret and dsherret/conditional-type-checks contributors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any ban-types

import {
  Assert,
  AssertFalse,
  AssertTrue,
  assertType,
  Has,
  IsAny,
  IsExact,
  IsNever,
  IsNullable,
  IsUnknown,
  NotHas,
} from "./types.ts";

// IsNullable
{
  // matching
  assertType<IsNullable<string | null>>(true);
  assertType<IsNullable<string | undefined>>(true);
  assertType<IsNullable<null | undefined>>(true); // maybe this shouldn't be true?

  // not matching
  assertType<IsNullable<string>>(false);
  assertType<IsNullable<any>>(false);
  assertType<IsNullable<never>>(false);
  assertType<IsNullable<unknown>>(false);
}

// IsExact
{
  // matching
  assertType<IsExact<string | number, string | number>>(true);
  assertType<IsExact<string | number | Date, string | number | Date>>(true);
  assertType<IsExact<string | undefined, string | undefined>>(true);
  assertType<IsExact<any, any>>(true); // ok to have any for both
  assertType<IsExact<unknown, unknown>>(true);
  assertType<IsExact<never, never>>(true);
  assertType<IsExact<{}, {}>>(true);
  assertType<IsExact<{ prop: string }, { prop: string }>>(true);
  assertType<IsExact<{ prop: { prop: string } }, { prop: { prop: string } }>>(
    true,
  );
  assertType<IsExact<{ prop: never }, { prop: never }>>(true);
  assertType<IsExact<{ prop: any }, { prop: any }>>(true);
  assertType<IsExact<{ prop: unknown }, { prop: unknown }>>(true);
  assertType<IsExact<typeof globalThis, typeof globalThis>>(true);

  // not matching
  assertType<IsExact<string | number | Date, string | number>>(false);
  assertType<IsExact<string, string | number>>(false);
  assertType<IsExact<string | undefined, string>>(false);
  assertType<IsExact<string | undefined, any | string>>(false);
  assertType<IsExact<any | string | undefined, string>>(false);
  assertType<IsExact<string, any>>(false);
  assertType<IsExact<string, unknown>>(false);
  assertType<IsExact<string, never>>(false);
  assertType<IsExact<never, never | string>>(false);
  assertType<IsExact<unknown, any>>(false);
  assertType<IsExact<never, any>>(false);
  assertType<IsExact<Date | typeof globalThis, Date>>(false);
  assertType<IsExact<{ name: string; other?: Date }, { name: string }>>(false);
  assertType<IsExact<{ prop: Date }, { prop: string }>>(false);
  assertType<IsExact<{ other?: Date }, { prop?: string }>>(false);
  assertType<IsExact<{ prop: { prop?: string } }, { prop: { prop: string } }>>(
    false,
  );
  assertType<IsExact<{ prop: any }, { prop: string }>>(false);
  assertType<IsExact<{ prop: any }, { prop: unknown }>>(false);
  assertType<IsExact<{ prop: any }, { prop: never }>>(false);
  assertType<IsExact<{ prop: unknown }, { prop: never }>>(false);
  assertType<IsExact<{ prop: { prop: unknown } }, { prop: { prop: any } }>>(
    false,
  );
  assertType<IsExact<{ prop: { prop: unknown } }, { prop: { prop: never } }>>(
    false,
  );
  assertType<IsExact<{ prop: { prop: any } }, { prop: { prop: never } }>>(
    false,
  );
  assertType<IsExact<{ prop: string }, { prop: never }>>(false);
  assertType<IsExact<{ prop: { prop: any } }, { prop: { prop: string } }>>(
    false,
  );
  assertType<
    IsExact<
      { prop: any } | { prop: string },
      { prop: number } | { prop: string }
    >
  >(false);
  assertType<IsExact<{ prop: string | undefined }, { prop?: string }>>(false); // these are different
}

// Has
{
  // matching
  assertType<Has<string | number, string>>(true);
  assertType<Has<number, number>>(true);
  assertType<Has<string | number, Date | string>>(true); // maybe?
  assertType<Has<any, number>>(true);
  assertType<Has<any, any>>(true);
  assertType<Has<any, unknown>>(true);
  assertType<Has<any, never>>(true);

  // not matching
  assertType<Has<string | number, Date>>(false);
  assertType<Has<string, number>>(false);
  assertType<Has<number, any>>(false);
  assertType<Has<unknown, any>>(false);
  assertType<Has<never, any>>(false);
}

// NotHas
{
  // matching
  assertType<NotHas<string | number, Date>>(true);
  assertType<NotHas<string, number>>(true);

  // not matching
  assertType<NotHas<string | number, string>>(false);
  assertType<NotHas<number, number>>(false);
  assertType<NotHas<string | number, Date | string>>(false); // should be true?
}

// IsAny
{
  // matching
  assertType<IsAny<any>>(true);

  // not matching
  assertType<IsAny<string>>(false);
  assertType<IsAny<unknown>>(false);
  assertType<IsAny<never>>(false);

  // tests for issue #3 (IsAny resolving to boolean)
  assertType<IsExact<IsAny<string>, false>>(true);
  assertType<IsExact<IsAny<5>, false>>(true);
}

// IsNever
{
  // matching
  assertType<IsNever<never>>(true);

  // not matching
  assertType<IsNever<string>>(false);
  assertType<IsNever<any>>(false);
  assertType<IsNever<unknown>>(false);
}

// IsUnknown
{
  // matching
  assertType<IsUnknown<unknown>>(true);

  // not matching
  assertType<IsUnknown<string>>(false);
  assertType<IsUnknown<any>>(false);
  assertType<IsUnknown<never>>(false);
}

// AssertTrue
{
  type test = AssertTrue<IsNever<never>>;
}

// AssertFalse
{
  type test = AssertFalse<IsNever<string>>;
}

// Assert
{
  type test =
    | Assert<Has<string | number, number>, true>
    | Assert<Has<string | number, Date>, false>;
}

// Recursive types
{
  type RecursiveType1 = string | number | Date | RecursiveType1[];
  assertType<IsExact<RecursiveType1, RecursiveType1>>(true);
  type RecursiveType2 = {
    a: string;
    prop: RecursiveType2;
    sub: {
      prop: RecursiveType2;
      other: RecursiveType1;
    };
  };
  assertType<IsExact<RecursiveType2, RecursiveType2>>(true);
  assertType<IsExact<RecursiveType1, RecursiveType2>>(false);
}
