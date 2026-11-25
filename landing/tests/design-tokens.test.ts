import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

function parseDesignFrontmatterColors(markdown: string): Map<string, string> {
  const match = markdown.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!match) {
    throw new Error("DESIGN.md frontmatter missing");
  }
  const colors = new Map<string, string>();
  let inColors = false;
  for (const rawLine of match[1].split(/\r?\n/)) {
    if (rawLine.trim() === "colors:") {
      inColors = true;
      continue;
    }
    if (inColors && !rawLine.startsWith("  ")) {
      inColors = false;
    }
    if (!inColors) continue;
    const entry = rawLine.match(/^\s+([a-zA-Z0-9]+):\s*"(#[0-9a-fA-F]{6})"/);
    if (entry) {
      colors.set(entry[1], entry[2].toLowerCase());
    }
  }
  return colors;
}

function parseThemeHexTokens(css: string): Map<string, string> {
  const theme = css.match(/@theme\s*\{([\s\S]*?)\}/);
  if (!theme) {
    throw new Error("@theme block missing from global.css");
  }
  const tokens = new Map<string, string>();
  for (const match of theme[1].matchAll(/--color-([a-z0-9-]+):\s*(#[0-9a-fA-F]{6});/g)) {
    tokens.set(match[1], match[2].toLowerCase());
  }
  return tokens;
}

function kebabToCamel(value: string): string {
  return value.replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

describe("landing design token parity", () => {
  test("all @theme hex color tokens in global.css match DESIGN.md", () => {
    const designMd = readFileSync(resolve(import.meta.dir, "../DESIGN.md"), "utf8");
    const globalCss = readFileSync(resolve(import.meta.dir, "../src/styles/global.css"), "utf8");

    const designColors = parseDesignFrontmatterColors(designMd);
    const themeColors = parseThemeHexTokens(globalCss);

    expect(themeColors.size).toBeGreaterThanOrEqual(20);
    for (const [token, hex] of themeColors) {
      const designKey = token === "accent-text" ? "accentHover" : kebabToCamel(token);
      expect(designColors.get(designKey)).toBe(hex);
    }
  });

  test("motion curve token in global.css matches DESIGN.md", () => {
    const designMd = readFileSync(resolve(import.meta.dir, "../DESIGN.md"), "utf8");
    const globalCss = readFileSync(resolve(import.meta.dir, "../src/styles/global.css"), "utf8");

    const curve = designMd.match(/^\s+curve:\s*"([^"]+)"/m)?.[1];
    const cssCurve = globalCss.match(/--ease-out-expo:\s*([^;]+);/)?.[1]?.trim();

    expect(curve).toBeDefined();
    expect(cssCurve).toBe(curve);
  });
});
