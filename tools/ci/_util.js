export const ARCHIVE_COUNT = 4;

export class CargoBuildManifest {
  #artifacts;

  constructor(path) {
    const manifestText = Deno.readTextFileSync(path);
    const manifest = manifestText.split("\n").filter((line) => line.length > 0)
      .map((line) => JSON.parse(line));
    this.#artifacts = manifest
      .filter((item) => item.reason === "compiler-artifact");
  }

  get bins() {
    return this.#artifacts.filter(({ target }) => target.kind.includes("bin"));
  }

  get cdylibs() {
    return this.#artifacts.filter(({ target }) =>
      target.kind.includes("cdylib")
    );
  }

  get #tests() {
    return this.#artifacts.filter((a) => a.profile.test);
  }

  get tests() {
    return this.#tests.filter((a) => !a.target.kind.includes("bench"));
  }

  get benches() {
    return this.#tests.filter((a) => a.target.kind.includes("bench"));
  }
}
