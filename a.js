import { escapeHTML } from "npm:@napi-rs/escape";
const escaped = escapeHTML(`<div>{props.getNumber()}</div>`);
console.log(escaped);
