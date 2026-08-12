export interface User {
  name: string;
  age: number;
}

export function greet(user: User): string {
  return `Hello, ${user.name}! You are ${user.age} years old.`;
}

export const DEFAULT_USER: User = { name: "Alice", age: 30 };

console.log(greet(DEFAULT_USER));
