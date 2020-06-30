const indent = (line: string, spaces: number): string => {
  return new Array(spaces).fill(" ").join("") + line;
};
export const stringify = (object: unknown, spaces = 0): string => {
  return computeLines(object, spaces)
    .map((line) => indent(line.content, line.indent))
    .join("\n");
};

type Line = {
  content: string;
  indent: number;
};
const computeLines = (object: unknown, indent: number): Line[] => {
  if (Array.isArray(object)) {
    return [
      { content: "[", indent },
      ...object.flatMap((o) =>
        appendCommaToLastLine(computeLines(o, indent + 2))
      ),
      { content: "]", indent },
    ];
  } else if (typeof object === "object" && object !== null) {
    return [
      { content: "{", indent },
      ...Object.entries(object).flatMap(([key, value]) => {
        const lines = computeLines(value, indent + 2).map<Line>(
          (line, index) => {
            if (index === 0) {
              return {
                content: `"${key}": ${line.content}`,
                indent: line.indent,
              };
            } else {
              return {
                content: line.content,
                indent: line.indent,
              };
            }
          }
        );
        return appendCommaToLastLine(lines);
      }),
      { content: "}", indent },
    ];
  } else if (typeof object === "symbol" || typeof object === "function") {
    return [{ content: object.toString(), indent }];
  } else {
    return [{ content: JSON.stringify(object), indent }];
  }
};

const appendCommaToLastLine = (lines: Line[]): Line[] => {
  return lines.map((line, index) => {
    // Append comma to last line
    if (index === lines.length - 1) {
      return { ...line, content: line.content + "," };
    } else {
      return line;
    }
  });
};
