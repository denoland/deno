export function debounce<T extends (...args: any[]) => void>(
  fn: T,
  delay: number,
): T {
  let timer: ReturnType<typeof setTimeout>;
  return ((...args: any[]) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delay);
  }) as T;
}

export function formatDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export async function fetchJSON<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`HTTP ${res.status}: ${res.statusText}`);
  }
  return res.json();
}

export function groupBy<T>(items: T[], key: keyof T): Record<string, T[]> {
  return items.reduce(
    (acc, item) => {
      const k = String(item[key]);
      if (!acc[k]) acc[k] = [];
      acc[k].push(item);
      return acc;
    },
    {} as Record<string, T[]>,
  );
}

export const ROLES = ["admin", "user", "guest"] as const;
export type Role = (typeof ROLES)[number];

// --- Type-aware lint demos ---
// These trigger rules that require TypeScript type information.

// no-floating-promises: Promise result is ignored (should be awaited or handled)
export function loadUsers() {
  fetchJSON("/api/users");
}

// await-thenable: Awaiting a non-Promise value is pointless
export async function getRole(): Promise<Role> {
  const role: Role = "admin";
  return await role;
}
