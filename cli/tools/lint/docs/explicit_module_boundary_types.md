Requires all module exports to have fully typed declarations

Having fully typed function arguments and return values clearly defines the
inputs and outputs of a module (known as the module boundary). This will make it
very clear to any users of the module how to supply inputs and handle outputs in
a type safe manner.

### Invalid:

```typescript
// Missing return type (e.g. void)
export function printDoc(doc: string, doubleSided: boolean) {
  return;
}

// Missing argument type (e.g. `arg` is of type string)
export const arrowFn = (arg): string => `hello ${arg}`;

// Missing return type (e.g. boolean)
export function isValid() {
  return true;
}
```

### Valid:

```typescript
// Typed input parameters and return value
export function printDoc(doc: string, doubleSided: boolean): void {
  return;
}

// Input of type string and a return value of type string
export const arrowFn = (arg: string): string => `hello ${arg}`;

// Though lacking a return type, this is valid as it is not exported
function isValid() {
  return true;
}
```
