// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toMatchObject()", () => {
  const house0 = {
    bath: true,
    bedrooms: 4,
    kitchen: {
      amenities: ["oven", "stove", "washer"],
      area: 20,
      wallColor: "white",
    },
  };
  const house1 = {
    bath: true,
    bedrooms: 4,
    kitchen: {
      amenities: ["oven", "stove"],
      area: 20,
      wallColor: "white",
    },
  };
  const desiredHouse = {
    bath: true,
    kitchen: {
      amenities: ["oven", "stove", "washer"],
      wallColor: "white",
    },
  };

  expect(house0).toMatchObject(desiredHouse);

  expect(house1).not.toMatchObject(desiredHouse);

  assertThrows(() => {
    expect(house1).toMatchObject(desiredHouse);
  }, AssertionError);

  assertThrows(() => {
    expect(house0).not.toMatchObject(desiredHouse);
  }, AssertionError);
});
