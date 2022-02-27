// This file should be excluded from formatting
Deno.test(
    { perms: { net: true } },
    async function fetchBodyUsedCancelStream() {
      const response = await fetch(
        "http://localhost:4545/fixture.json",
      );
      assert(response.body !== null);
  
      assertEquals(response.bodyUsed, false);
      const promise = response.body.cancel();
      assertEquals(response.bodyUsed, true);
      await promise;
    },
);