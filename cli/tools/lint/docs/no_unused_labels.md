Disallows unused labels.

A label that is declared but never used is most likely developer's mistake. If
that label is meant to be used, then write a code so that it will be used.
Otherwise, remove the label.

### Invalid:

```typescript
LABEL1:
while (true) {
  console.log(42);
}

LABEL2:
for (let i = 0; i < 5; i++) {
  console.log(42);
}

LABEL3:
for (const x of xs) {
  console.log(x);
}
```

### Valid:

```typescript
LABEL1:
while (true) {
  console.log(42);
  break LABEL1;
}

LABEL2:
for (let i = 0; i < 5; i++) {
  console.log(42);
  continue LABEL2;
}

for (const x of xs) {
  console.log(x);
}
```
