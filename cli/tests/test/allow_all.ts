import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

// TODO test all the things

Deno.test({
  name: `readGranted`,
  async fn() {
    const status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "granted");
  },
});

Deno.test({
  name: `readRevoked`,
  permissions: {
    read: false,
  },
  async fn() {
    const status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "prompt");
  },
});

Deno.test({
  name: `readGrantedAgain`,
  permissions: {
    read: [],
  },
  async fn() {
    const status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "granted");
  },
});

Deno.test({
  name: `readRevokedAgain`,
  permissions: {
    read: false,
  },
  async fn() {
    const status = await Deno.permissions.query({ name: "read" });
    assertEquals(status.state, "prompt");
  },
});
