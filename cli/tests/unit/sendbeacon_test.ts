// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
    assert,
    assertRejects,
  } from "./test_util.ts";
  
  Deno.test(
    "sendBeacon() empty parameters",
    { permissions: { net: true } },
    async () => {
      await assertRejects(
        async () => {
          // @ts-ignore no parameters provided
          await navigator.sendBeacon();
        },
      );
    },
  );
  
  Deno.test(
    "sendBeacon() echo",
    { permissions: { net: true } },
    async () => {
      assert(
        await navigator.sendBeacon(
          "http://localhost:4545/echo_server",
          "Hello World",
        ),
      );
    },
  );
  
  Deno.test(
    "sendBeacon() symbol passed in as data parameter",
    { permissions: { net: true } },
    async () => {
      await assertRejects(
        async () => {
          await navigator.sendBeacon(
            "http://localhost:4545/echo_server",
            // @ts-ignore data parameter not provided
            Symbol("[[Symbol]]"),
          );
        },
      );
    },
  );
  