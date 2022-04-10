/// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { GitLogOutput } from "./helpers.ts";

export class VersionReleaseText {
  #fullText: string;

  constructor(fullText: string) {
    this.#fullText = fullText;
  }

  get fullText() {
    return this.#fullText.trim();
  }

  get version() {
    const version = /\b[0-9]+\.[0-9]+\.[0-9]+\b/.exec(this.#fullText);
    if (version == null) {
      throw new Error(`Could not find version in ${this.#fullText}.`);
    }
    return version[0];
  }
}

/** Helpers for dealing with a Releases.md file. */
export class ReleasesMdFile {
  #filePath: string;
  #fileText: string;

  constructor(filePath: string) {
    this.#filePath = filePath;
    this.#fileText = Deno.readTextFileSync(filePath);
  }

  get filePath() {
    return this.#filePath;
  }

  get fileText() {
    return this.#fileText;
  }

  updateWithGitLog(
    { gitLog, version, date }: {
      gitLog: GitLogOutput;
      version: string;
      date?: Date;
    },
  ) {
    const insertText = getInsertText();
    this.#updateText(this.#fileText.replace(/^### /m, insertText + "\n\n### "));

    function getInsertText() {
      const formattedGitLog = gitLog.formatForReleaseMarkdown();
      const formattedDate = getFormattedDate(date ?? new Date());

      return `### ${version} / ${formattedDate}\n\n` +
        `${formattedGitLog}`;

      function getFormattedDate(date: Date) {
        const formattedMonth = padTwoDigit(date.getMonth() + 1);
        const formattedDay = padTwoDigit(date.getDate());
        return `${date.getFullYear()}.${formattedMonth}.${formattedDay}`;

        function padTwoDigit(val: number) {
          return val.toString().padStart(2, "0");
        }
      }
    }
  }

  getLatestReleaseText() {
    const version = this.getAllReleaseTexts().next().value;
    if (version instanceof VersionReleaseText) {
      return version;
    } else {
      throw new Error("Expected at least one version.");
    }
  }

  *getAllReleaseTexts() {
    const matches = this.#fileText.matchAll(/^### /mg);
    let lastIndex = matches.next().value.index!;
    for (const match of matches) {
      yield new VersionReleaseText(
        this.#fileText.substring(lastIndex, match.index!),
      );
      lastIndex = match.index;
    }
    yield new VersionReleaseText(
      this.#fileText.substring(lastIndex, this.#fileText.length),
    );
  }

  #updateText(newText: string) {
    this.#fileText = newText;
    Deno.writeTextFileSync(this.#filePath, this.#fileText);
  }
}
