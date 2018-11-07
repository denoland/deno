import { listen } from "./server.ts";

const s = listen("0.0.0.0:4500");
s.serve();
