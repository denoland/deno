// This file contains unit tests for the Deno.exit.code API
import { assertEquals, assertThrows } from "../../testing/asserts.ts";

Deno.test({
  name: "Deno.exit.code getter and setter",
  fn() {
    // Initial value is 0
    assertEquals(Deno.exit.code, 0);

    // Set a new value
    Deno.exit.code = 5;
    assertEquals(Deno.exit.code, 5);

    // Reset to initial value
    Deno.exit.code = 0;
    assertEquals(Deno.exit.code, 0);

    // Throws on non-number values
    assertThrows(() => {
      // @ts-expect-error Testing for runtime error
      Deno.exit.code = "not a number";
    }, TypeError, "Exit code must be a number.");
  },
});

Deno.test({
  name: "Setting Deno.exit.code does not cause an immediate exit",
  fn() {
    let exited = false;
    const originalExit = Deno.exit;
    Deno.exit = () => {
      exited = true;
    };

    Deno.exit.code = 1;
    assertEquals(exited, false);

    Deno.exit = originalExit;
  },
});

Deno.test({
  name: "Retrieving the set exit code before process termination",
  fn() {
    Deno.exit.code = 42;
    assertEquals(Deno.exit.code, 42);

    // Reset to initial value
    Deno.exit.code = 0;
  },
});
