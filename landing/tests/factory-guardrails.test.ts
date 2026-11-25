import { Database } from "bun:sqlite";
import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const LANDING_ROOT = resolve(import.meta.dir, "..");
const MAX_NEW_FILE_LINES = 450;
const MAX_LINT_IGNORES = 2;

const LEGACY_LARGE_FILES: Record<string, number> = {
  "src/components/islands/DockGenie.tsx": 454,
  "src/components/islands/IslandStage.tsx": 515,
  "src/components/islands/OttoSettings.tsx": 661,
  "src/components/islands/notch/Panels.tsx": 467,
};

function collectFiles(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      collectFiles(full, acc);
    } else {
      acc.push(full);
    }
  }
  return acc.sort();
}

describe("landing anti-slop guardrails", () => {
  const prodFiles = [...collectFiles(join(LANDING_ROOT, "src")), ...collectFiles(join(LANDING_ROOT, "functions"))];

  test("no new production file exceeds 450 lines and legacy large files do not grow", () => {
    for (const file of prodFiles) {
      const rel = relative(LANDING_ROOT, file).replace(/\\/g, "/");
      const lines = readFileSync(file, "utf8").split(/\r?\n/).length;
      const ceiling = LEGACY_LARGE_FILES[rel] ?? MAX_NEW_FILE_LINES;
      expect(lines, `${rel} (${lines} lines) exceeds ceiling ${ceiling}`).toBeLessThanOrEqual(ceiling);
    }
  });

  test("production code contains zero explicit any casts and bounded lint suppressions", () => {
    let ignoreCount = 0;
    for (const file of prodFiles) {
      const rel = relative(LANDING_ROOT, file).replace(/\\/g, "/");
      const content = readFileSync(file, "utf8");
      const anyMatches = content.match(/:\s*any\b|\bas\s+any\b/g) ?? [];
      expect(anyMatches.length, `${rel} must not introduce explicit any`).toBe(0);
      ignoreCount += (content.match(/biome-ignore|@ts-ignore|@ts-expect-error|eslint-disable/g) ?? []).length;
    }
    expect(ignoreCount).toBeLessThanOrEqual(MAX_LINT_IGNORES);
  });

  test("D1 SQL migrations apply cleanly in SQLite and support waitlist & rate-limit queries", () => {
    const migrationsDir = join(LANDING_ROOT, "migrations");
    const sqlFiles = collectFiles(migrationsDir).filter((file) => file.endsWith(".sql"));
    expect(sqlFiles.length).toBeGreaterThan(0);

    const db = new Database(":memory:");
    try {
      for (const file of sqlFiles) {
        db.run(readFileSync(file, "utf8"));
      }

      db.query("INSERT INTO waitlist (email, source, token_hash, platform, usage) VALUES (?, ?, ?, ?, ?)").run(
        "user@example.com",
        "hero",
        "abc123hash",
        "win11",
        "daily",
      );

      const saved = db
        .query("SELECT email, source, token_hash, created_at FROM waitlist WHERE email = ?")
        .get("user@example.com") as { email: string; source: string; token_hash: string; created_at: string } | null;
      expect(saved?.email).toBe("user@example.com");
      expect(saved?.source).toBe("hero");
      expect(saved?.created_at).toBeTruthy();

      const firstHit = db
        .query(
          "INSERT INTO rate_limits (key, bucket, count) VALUES (?, ?, 1) ON CONFLICT(key) DO UPDATE SET count = count + 1 RETURNING count",
        )
        .get("127.0.0.1:100", 100) as { count: number };
      const secondHit = db
        .query(
          "INSERT INTO rate_limits (key, bucket, count) VALUES (?, ?, 1) ON CONFLICT(key) DO UPDATE SET count = count + 1 RETURNING count",
        )
        .get("127.0.0.1:100", 100) as { count: number };
      expect(firstHit.count).toBe(1);
      expect(secondHit.count).toBe(2);
    } finally {
      db.close();
    }
  });
});
