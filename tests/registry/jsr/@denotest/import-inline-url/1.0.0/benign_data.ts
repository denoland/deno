import staticValue from "data:application/javascript,export%20default%20%22static%22";

const { default: dynamicValue } = await import(
  "data:application/javascript,export%20default%20%22dynamic%22"
);

console.log(staticValue, dynamicValue);
