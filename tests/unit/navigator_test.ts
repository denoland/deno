// Copyright 2018-2026 the Deno authors. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(function navigatorNumCpus() {
  assert(navigator.hardwareConcurrency > 0);
});

Deno.test(function navigatorUserAgent() {
  const pattern = /Deno\/\d+\.\d+\.\d+/;
  assert(pattern.test(navigator.userAgent));
});

Deno.test(function navigatorUserAgentData() {
  const uaData = navigator.userAgentData;
  assert(uaData instanceof NavigatorUAData);

  // Low-entropy values.
  assertEquals(uaData.mobile, false);
  assert(Array.isArray(uaData.brands));
  assert(uaData.brands.length > 0);
  const deno = uaData.brands.find((b) => b.brand === "Deno");
  assert(deno !== undefined, "expected a Deno brand");
  assert(/^\d+$/.test(deno!.version), "brand version should be the major");
  assert(typeof uaData.platform === "string" && uaData.platform.length > 0);
});

Deno.test(function navigatorUserAgentDataIsSingleton() {
  assert(navigator.userAgentData === navigator.userAgentData);
});

Deno.test(function navigatorUserAgentDataIllegalConstructor() {
  assertThrows(
    () => new NavigatorUAData(),
    TypeError,
  );
});

Deno.test(function navigatorUserAgentDataToJSON() {
  const json = navigator.userAgentData.toJSON();
  assertEquals(Object.keys(json).sort(), ["brands", "mobile", "platform"]);
  assertEquals(json.mobile, false);
});

Deno.test(async function navigatorUserAgentDataHighEntropyValues() {
  const values = await navigator.userAgentData.getHighEntropyValues([
    "architecture",
    "bitness",
    "model",
    "platformVersion",
    "uaFullVersion",
    "fullVersionList",
  ]);

  // Low-entropy values are always present.
  assertEquals(values.mobile, false);
  assert(Array.isArray(values.brands));
  assert(typeof values.platform === "string");

  // Requested high-entropy values.
  assert(typeof values.architecture === "string");
  assertEquals(values.bitness, "64");
  assertEquals(values.model, "");
  assertEquals(values.platformVersion, "");
  assert(/^\d+\.\d+\.\d+/.test(values.uaFullVersion!));
  assert(Array.isArray(values.fullVersionList));
  const deno = values.fullVersionList!.find((b) => b.brand === "Deno");
  assert(deno !== undefined);
  assert(/^\d+\.\d+\.\d+/.test(deno!.version));
});

Deno.test(async function navigatorUserAgentDataIgnoresUnknownHints() {
  const values = await navigator.userAgentData.getHighEntropyValues([
    "notARealHint",
  ]);
  assertEquals(Object.keys(values).sort(), ["brands", "mobile", "platform"]);
});

Deno.test(async function navigatorUserAgentDataEmptyHints() {
  const values = await navigator.userAgentData.getHighEntropyValues([]);
  assertEquals(Object.keys(values).sort(), ["brands", "mobile", "platform"]);
});

Deno.test(async function navigatorUserAgentDataRequiresArgument() {
  await assertRejects(
    // @ts-expect-error: testing missing required argument
    () => navigator.userAgentData.getHighEntropyValues(),
    TypeError,
  );
});

Deno.test(function navigatorUserAgentDataInspect() {
  const out = Deno.inspect(navigator.userAgentData);
  assertStringIncludes(out, "NavigatorUAData");
  assertStringIncludes(out, "brands");
  assertStringIncludes(out, "platform");
});
