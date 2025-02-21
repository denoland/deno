import { assertEquals } from "jsr:@std/assert"

Deno.test({ name: "Some test" }, () => {
    assertEquals(1, 1)
})