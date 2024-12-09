Requires TODOs to be annotated with either a user tag (`@user`) or an issue
reference (`#issue`).

TODOs without reference to a user or an issue become stale with no easy way to
get more information.

### Invalid:

```typescript
// TODO Improve calc engine
export function calcValue(): number {}
```

```typescript
// TODO Improve calc engine (@djones)
export function calcValue(): number {}
```

```typescript
// TODO Improve calc engine (#332)
export function calcValue(): number {}
```

### Valid:

```typescript
// TODO(djones) Improve calc engine
export function calcValue(): number {}
```

```typescript
// TODO(@djones) Improve calc engine
export function calcValue(): number {}
```

```typescript
// TODO(#332)
export function calcValue(): number {}
```

```typescript
// TODO(#332) Improve calc engine
export function calcValue(): number {}
```
