import { existsSync, readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";

const root = realpathSync.native(process.cwd());
if (process.cwd() !== root) {
  process.chdir(root);
}

const child = Bun.spawnSync(["bun", "x", "astro", "build"], {
  cwd: root,
  stdio: ["inherit", "inherit", "inherit"],
});

if (child.exitCode !== 0) {
  process.exit(child.exitCode ?? 1);
}

const requiredPages = ["dist/index.html", "dist/privacy/index.html"];
for (const page of requiredPages) {
  const pagePath = resolve(root, page);
  if (!existsSync(pagePath)) {
    console.error(`Missing built page: ${page}`);
    process.exit(1);
  }
  const html = readFileSync(pagePath, "utf8");
  const matches = [...html.matchAll(/<link\b[^>]*\brel=["']stylesheet["'][^>]*\bhref=["']([^"']+)["']/gi)];
  if (matches.length === 0) {
    console.error(`Built HTML is missing stylesheet links: ${page}`);
    process.exit(1);
  }
  for (const match of matches) {
    const href = match[1].replace(/^\/+/, "");
    const cssPath = resolve(root, "dist", href);
    if (!existsSync(cssPath)) {
      console.error(`Referenced stylesheet does not exist (${page}): ${match[1]}`);
      process.exit(1);
    }
  }
}

console.log(`Verified stylesheet links in ${requiredPages.join(", ")}`);
