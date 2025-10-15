//@ts-expect-error intentionally overwriting Event
globalThis.Event = class {};
export default {};
