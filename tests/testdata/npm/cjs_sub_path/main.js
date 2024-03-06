// this package will require a subpath like "ajv/dist/compile/codegen"
// and also get the parent directory index.js file using require("..")
import Ajv from "npm:ajv@~8.11";
import addFormats from "npm:ajv-formats@2.1.1";
import { expect } from "npm:chai@4.3";

const ajv = new Ajv();
addFormats(ajv);

const schema = {
  type: "string",
  format: "date",
  formatMinimum: "2016-02-06",
  formatExclusiveMaximum: "2016-12-27",
};
const validate = ajv.compile(schema);

expect(validate("2016-02-06")).to.be.true;
expect(validate("2016-02-05")).to.be.false;

console.log("Fini");
