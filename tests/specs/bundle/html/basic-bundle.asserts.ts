import { assertFileContains } from "./assert-helpers.ts";
const re = /src="\.\/index-[^\.]+\.js"/;

assertFileContains("./dist/index.html", re);
