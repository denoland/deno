import console from "node:console";
import { context, propagation } from "npm:@opentelemetry/api@1.9.0";

const getter = {
  get(carrier: Map<string, string>, key: string) {
    return carrier.get(key);
  },
  keys(carrier: Map<string, string>) {
    return Array.from(carrier.keys());
  },
};
const setter = {
  set(carrier: Map<string, string>, key: string, value: string) {
    carrier.set(key, value);
  },
};

const carrier = new Map<string, string>([
  ["baggage", "key1=value1;meta1,key2=value2"],
]);
const ctx = propagation.extract(context.active(), carrier, getter);
// deno-lint-ignore no-explicit-any
const baggage = propagation.getBaggage(ctx) as any;

// getEntry returns the value and preserves the metadata object's toString().
const entry = baggage.getEntry("key1");
// getEntry returns a fresh copy: mutating it must not affect the baggage.
entry.value = "mutated";

// setEntry / removeEntry / removeEntries / clear are immutable: they return a
// new baggage and leave the receiver untouched.
const withKey3 = baggage.setEntry("key3", { value: "value3" });
const withoutKey1 = baggage.removeEntry("key1");
const withoutBoth = baggage.removeEntries("key1", "key2");
const cleared = baggage.clear();

// metadata survives a round-trip through inject.
const newCarrier = new Map<string, string>();
propagation.inject(
  propagation.setBaggage(ctx, baggage),
  newCarrier,
  setter,
);

console.log(JSON.stringify({
  getEntryValue: baggage.getEntry("key1").value,
  getEntryMetadata: baggage.getEntry("key1").metadata.toString(),
  getEntryMissing: baggage.getEntry("nope") ?? null,
  getAllEntries: baggage.getAllEntries().map((
    // deno-lint-ignore no-explicit-any
    [key, value]: [string, any],
  ) => [key, value.value, value.metadata?.toString() ?? null]),
  copyIsolated: baggage.getEntry("key1").value === "value1",
  setOriginalUnchanged: baggage.getEntry("key3") === undefined,
  setNewHasKey3: withKey3.getEntry("key3").value,
  removeOriginalUnchanged: baggage.getEntry("key1") !== undefined,
  removeNewMissingKey1: withoutKey1.getEntry("key1") === undefined,
  removeEntriesLen: withoutBoth.getAllEntries().length,
  clearedLen: cleared.getAllEntries().length,
  clearOriginalUnchanged: baggage.getAllEntries().length,
  injected: newCarrier.get("baggage"),
}));
