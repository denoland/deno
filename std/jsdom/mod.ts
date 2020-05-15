import { jsdom as instance } from "./dist/jsdom.js";
import { jsdom } from "./types/jsdom.ts";

export const JSDOM = instance.JSDOM as jsdom.JSDOM;
