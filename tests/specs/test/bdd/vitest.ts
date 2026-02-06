import { assertEquals } from "@std/assert";

Deno.describe("foo", () => {
  const s = { befAll: 0, befEach: 0, afAll: 0, afEach: 0 };

  Deno.beforeAll(() => {
    s.befAll++;
  });
  Deno.beforeEach(() => {
    s.befEach++;
  });
  Deno.afterAll(() => {
    s.afAll++;
  });
  Deno.afterEach(() => {
    s.afEach++;
  });

  Deno.test("add", () => {
    assertEquals(s, { befAll: 1, befEach: 1, afAll: 0, afEach: 0 });
  });

  Deno.test("add2", () => {
    assertEquals(s, { befAll: 1, befEach: 2, afAll: 0, afEach: 1 });
  });

  const s2 = { befAll: 0, befEach: 0, afAll: 0, afEach: 0 };
  Deno.describe("bar", () => {
    Deno.beforeAll(() => {
      s2.befAll++;
    });
    Deno.beforeEach(() => {
      s2.befEach++;
    });
    Deno.afterAll(() => {
      s2.afAll++;
    });
    Deno.afterEach(() => {
      s2.afEach++;
    });

    Deno.test("add3", () => {
      assertEquals(s, { befAll: 1, befEach: 3, afAll: 0, afEach: 2 });
      assertEquals(s2, {
        befAll: 1,
        befEach: 1,
        afAll: 0,
        afEach: 0,
      });
    });
    Deno.test("add4", () => {
      assertEquals(s, { befAll: 1, befEach: 4, afAll: 0, afEach: 3 });
      assertEquals(s2, {
        befAll: 1,
        befEach: 2,
        afAll: 0,
        afEach: 1,
      });
    });
  });

  Deno.test("add5", () => {
    assertEquals(s, { befAll: 1, befEach: 5, afAll: 0, afEach: 4 });

    assertEquals(s2, { befAll: 1, befEach: 2, afAll: 1, afEach: 2 });
  });
});
