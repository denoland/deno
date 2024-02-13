// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/assert_equals.ts";
import { UserAgent } from "./user_agent.ts";

Deno.test({
  name: "UserAgent.prototype.browser",
  async fn(t) {
    const specs = (await import("./testdata/user_agent/browser-all.json", {
      assert: { type: "json" },
    })).default;
    for (const { desc, ua, expect: { major, name, version } } of specs) {
      await t.step({
        name: desc,
        fn() {
          const actual = new UserAgent(ua);
          assertEquals(actual.browser, {
            major: major === "undefined" ? undefined : major,
            name: name === "undefined" ? undefined : name,
            version: version === "undefined" ? undefined : version,
          });
        },
      });
    }
  },
});

Deno.test({
  name: "UserAgent.prototype.cpu",
  async fn(t) {
    const specs = (await import("./testdata/user_agent/cpu-all.json", {
      assert: { type: "json" },
    })).default;
    for (const { desc: name, ua, expect } of specs) {
      await t.step({
        name,
        fn() {
          const actual = new UserAgent(ua);
          assertEquals(actual.cpu, expect);
        },
      });
    }
  },
});

Deno.test({
  name: "UserAgent.prototype.device",
  async fn(t) {
    const specs = (await import("./testdata/user_agent/device-all.json", {
      assert: { type: "json" },
    })).default;
    for (const { desc: name, ua, expect: { vendor, model, type } } of specs) {
      await t.step({
        name,
        fn() {
          const actual = new UserAgent(ua);
          assertEquals(actual.device, {
            vendor: vendor === "undefined" ? undefined : vendor,
            model: model === "undefined" ? undefined : model,
            type: type === "undefined" ? undefined : type,
          });
        },
      });
    }
  },
});

Deno.test({
  name: "UserAgent.prototype.engine",
  async fn(t) {
    const specs = (await import("./testdata/user_agent/engine-all.json", {
      assert: { type: "json" },
    })).default;
    for (const { desc, ua, expect: { name, version } } of specs) {
      await t.step({
        name: desc,
        fn() {
          const actual = new UserAgent(ua);
          assertEquals(actual.engine, {
            name: name === "undefined" ? undefined : name,
            version: version === "undefined" ? undefined : version,
          });
        },
      });
    }
  },
});

Deno.test({
  name: "UserAgent.prototype.os",
  async fn(t) {
    const specs = (await import("./testdata/user_agent/os-all.json", {
      assert: { type: "json" },
    })).default;
    for (const { desc, ua, expect: { name, version } } of specs) {
      await t.step({
        name: desc,
        fn() {
          const actual = new UserAgent(ua);
          assertEquals(actual.os, {
            name: name === "undefined" ? undefined : name,
            version: version === "undefined" ? undefined : version,
          });
        },
      });
    }
  },
});
