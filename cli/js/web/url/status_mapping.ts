// Ported from tr46 v2.0.2
// https://github.com/jsdom/tr46/tree/v2.0.2
// Copyright 2015-2020 by Sebastian Mayr. All rights reserved. MIT licence.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export enum STATUS_MAPPING {
  mapped = 1,
  valid = 2,
  disallowed = 3,
  // eslint-disable-next-line @typescript-eslint/camelcase
  disallowed_STD3_valid = 4,
  // eslint-disable-next-line @typescript-eslint/camelcase
  disallowed_STD3_mapped = 5,
  deviation = 6,
  ignored = 7,
}
