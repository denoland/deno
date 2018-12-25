import { stderr } from "deno";

stderr.write(new TextEncoder().encode("x"));
