import { existsSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const LANDING_ROOT = resolve(import.meta.dir, "..");
const DIST_DIR = join(LANDING_ROOT, "dist");

const REQUIRED_PAGES = ["index.html", "privacy/index.html"];

function assert(condition: unknown, message: string): void {
  if (!condition) {
    throw new Error(`SMOKE_FAIL: ${message}`);
  }
}

function verifyBuiltPages(): number {
  let checkedAssets = 0;
  assert(existsSync(DIST_DIR), "dist/ directory does not exist; run bun run build first");

  for (const page of REQUIRED_PAGES) {
    const pagePath = join(DIST_DIR, page);
    assert(existsSync(pagePath), `missing built page ${page}`);
    const html = readFileSync(pagePath, "utf8");
    assert(html.includes("<!DOCTYPE html>") || html.includes("<!doctype html>"), `${page} missing DOCTYPE`);
    assert(html.includes("<title>"), `${page} missing <title>`);
    assert(!html.includes("AstroError") && !html.includes("UnhandledRejection"), `${page} contains error marker`);

    const assetRefs = [...html.matchAll(/(?:src|href)="(\/_astro\/[^"?#]+)"/g)].map((m) => m[1]);
    for (const ref of new Set(assetRefs)) {
      const diskPath = join(DIST_DIR, ref.slice(1));
      assert(existsSync(diskPath), `${page} references missing asset ${ref}`);
      assert(statSync(diskPath).size > 0, `${page} references empty asset ${ref}`);
      checkedAssets += 1;
    }
  }

  const indexHtml = readFileSync(join(DIST_DIR, "index.html"), "utf8");
  const islandCount = (indexHtml.match(/<astro-island\b/g) ?? []).length;
  assert(islandCount >= 3, `index.html expected >= 3 hydrated astro-island elements, found ${islandCount}`);

  console.log(
    `LANDING SMOKE PASSED: ${REQUIRED_PAGES.length} pages, ${islandCount} astro islands, ${checkedAssets} bundled assets verified`,
  );
  return checkedAssets;
}

verifyBuiltPages();
