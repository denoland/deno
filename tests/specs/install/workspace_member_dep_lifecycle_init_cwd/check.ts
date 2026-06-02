// The dependency is declared by both the `frontend` and `backend` members, so
// its postinstall script must run once per member with INIT_CWD pointing at
// each member directory. Verify the marker landed in both members and not at
// the workspace root.
console.log("marker in frontend:", existsSync("frontend/init-cwd-marker.txt"));
console.log("marker in backend:", existsSync("backend/init-cwd-marker.txt"));
console.log("marker in root:", existsSync("init-cwd-marker.txt"));

function existsSync(path: string): boolean {
  try {
    Deno.statSync(path);
    return true;
  } catch {
    return false;
  }
}
