// A realistic Deno application using jsr, npm, URL, and assertion imports
import { Hono } from "npm:hono@4";
import { z } from "npm:zod@3";
import { sprintf } from "jsr:@std/fmt@1/printf";
import { join } from "jsr:@std/path@1/join";
import { assert } from "jsr:@std/assert@1";
import greeting from "./greeting.txt" with { type: "text" };
import favicon from "./favicon.png" with { type: "bytes" };

// --- Schema definitions using zod (npm) ---

const UserSchema = z.object({
  name: z.string().min(1),
  email: z.string().email(),
  age: z.number().int().positive(),
});

type User = z.infer<typeof UserSchema>;

// --- Application setup using Hono (npm) ---

const app = new Hono();

app.get("/", (c) => {
  return c.text(greeting);
});

app.get("/favicon.ico", (c) => {
  // favicon is Uint8Array (from bytes import)
  const size: number = favicon.byteLength;
  return c.text(`favicon: ${size} bytes`);
});

app.get("/hello/:name", (c) => {
  const name = c.req.param("name");
  // Using jsr:@std/fmt sprintf
  const message: string = sprintf("Hello, %s! Welcome.", name);
  return c.text(message);
});

app.post("/users", async (c) => {
  const body = await c.req.json();
  const user: User = UserSchema.parse(body);

  // Using jsr:@std/path join
  const dataPath: string = join(Deno.cwd(), "data", "users.json");

  // Using jsr:@std/assert
  assert(user.name.length > 0, "Name must not be empty");

  return c.json({ created: user, storedAt: dataPath });
});

// --- Deno APIs ---

const port = Number(Deno.env.get("PORT") ?? "8000");

console.log(sprintf("Server starting on port %d", port));
console.log(sprintf("Favicon size: %d bytes", favicon.byteLength));

Deno.serve({ port }, app.fetch);
