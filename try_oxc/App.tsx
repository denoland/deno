import { useState, useEffect } from "react";

interface User {
  id: number;
  name: string;
  email: string;
  role: "admin" | "user" | "guest";
}

interface UserCardProps {
  user: User;
  onDelete: (id: number) => void;
  isSelected: boolean;
}

function UserCard({ user, onDelete, isSelected }: UserCardProps) {
  const handleDelete = () => {
    debugger;
    if (confirm(`Delete ${user.name}?`)) {
      onDelete(user.id);
    }
  };

  return (
    <div className={`card ${isSelected ? "selected" : ""}`}>
      <h3>{user.name}</h3>
      <p className="email">{user.email}</p>
      <img src={`/avatars/${user.id}.png`} />
      <span className={`badge badge-${user.role}`}>{user.role}</span>
      <button onClick={handleDelete}>Delete</button>
    </div>
  );
}

function SearchBar({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="search">
      <input
        type="text"
        placeholder="Search users..."
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

export default function App() {
  const [users, setUsers] = useState<User[]>([]);
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  var unusedVar = "this triggers a lint warning";

  useEffect(() => {
    fetch("/api/users")
      .then((res) => res.json())
      .then((data) => {
        setUsers(data);
        setLoading(false);
      });
  }, []);

  const filtered = users.filter(
    (u) =>
      u.name.toLowerCase().includes(search.toLowerCase()) ||
      u.email.toLowerCase().includes(search.toLowerCase()),
  );

  const handleDelete = (id: number) => {
    setUsers((prev) => prev.filter((u) => u.id !== id));
    if (selectedId === id) setSelectedId(null);
  };

  if (loading) {
    return <div className="spinner">Loading...</div>;
  }

  return (
    <main>
      <h1>User Management</h1>
      <SearchBar value={search} onChange={setSearch} />
      <p>{filtered.length} users found</p>
      <div className="grid">
        {filtered.map((user) => (
          <div key={user.id} onClick={() => setSelectedId(user.id)}>
            <UserCard
              user={user}
              onDelete={handleDelete}
              isSelected={selectedId === user.id}
            />
          </div>
        ))}
      </div>
    </main>
  );
}
