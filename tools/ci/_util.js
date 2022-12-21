/**
 * Describes into how many separate archives, artifacts
 * produced by CI should split.
 */
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

  /** Return all artifacts that are executable binaries. */
  get bins() {
    return this.#artifacts.filter(({ target }) => target.kind.includes("bin"));
  }

  /** Return all artifacts that are dynamic libraries. */
  get cdylibs() {
    return this.#artifacts.filter(({ target }) =>
      target.kind.includes("cdylib")
    );
  }

  get #tests() {
    return this.#artifacts.filter((a) => a.profile.test);
  }

  /** Return all artifacts that are test binaries. */
  get tests() {
    return this.#tests.filter((a) => !a.target.kind.includes("bench"));
  }

  /** Return all artifacts that are benchmark binaries. */
  get benches() {
    return this.#tests.filter((a) => a.target.kind.includes("bench"));
  }
}
