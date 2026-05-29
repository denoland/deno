interface User {
  name: string;
  balance: number;
}

/**
 * Greets the user and shows their balance.
 * @param user - The user to greet.
 * @returns A greeting message with the user's name and balance.
 */
export function greet(user: User): string {
  return `Hello ${user.name}, you have $${user.balance.toFixed(2)}`;
}
