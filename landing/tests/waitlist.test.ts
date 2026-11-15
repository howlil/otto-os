import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { Database } from "bun:sqlite";
import { describe, expect, test } from "bun:test";

import { onRequestPost } from "../functions/api/waitlist";
import { WAITLIST_PLATFORMS, WAITLIST_USAGES } from "../src/domain/waitlist";

type Row = { email: string; source: string | null; token_hash: string; platform?: string; usage?: string };

function createDb() {
  const rows = new Map<string, Row>();
  const rateCounts = new Map<string, number>();

  const db = {
    prepare(sql: string) {
      let args: unknown[] = [];
      const statement = {
        bind(...values: unknown[]) {
          args = values;
          return statement;
        },
        async first<T>() {
          if (sql.startsWith("INSERT INTO rate_limits")) {
            const [key] = args as string[];
            const count = (rateCounts.get(key) ?? 0) + 1;
            rateCounts.set(key, count);
            return { count } as T;
          }
          return null;
        },
        async run() {
          if (sql.startsWith("DELETE FROM rate_limits")) {
            return { meta: { changes: 0 } };
          }
          if (sql.startsWith("UPDATE waitlist")) {
            const [platform, usage, email, hash] = args as string[];
            const row = rows.get(email);
            if (row && row.token_hash === hash) {
              row.platform = platform;
              row.usage = usage;
              return { meta: { changes: 1 } };
            }
            return { meta: { changes: 0 } };
          }
          const [email, source, tokenHash] = args as [string, string | null, string];
          if (rows.has(email)) return { meta: { changes: 0 } };
          rows.set(email, { email, source, token_hash: tokenHash });
          return { meta: { changes: 1 } };
        },
      };
      return statement;
    },
  };

  return { db: db as unknown as D1Database, rows, rateCounts };
}

type CallOptions = { ip?: string; contentType?: string };

async function call(db: D1Database, body: unknown, options: CallOptions = {}) {
  const request = new Request("https://example.test/api/waitlist", {
    method: "POST",
    headers: {
      "Content-Type": options.contentType ?? "application/json",
      "cf-connecting-ip": options.ip ?? "203.0.113.7",
    },
    body: typeof body === "string" ? body : JSON.stringify(body),
  });
  const response = await onRequestPost({ request, env: { DB: db } } as never);
  return { status: response.status, body: (await response.json()) as Record<string, unknown> };
}

const PROFILE = { platform: "Windows 11 22H2 atau lebih baru", usage: "Jarang" };

describe("join", () => {
  test("new email returns a token and stores only its hash", async () => {
    const { db, rows } = createDb();
    const result = await call(db, { email: "Test@Example.com", source: "hero" });
    expect(result.status).toBe(200);
    expect(typeof result.body.token).toBe("string");
    const stored = rows.get("test@example.com")!;
    expect(stored.token_hash).toHaveLength(64);
    expect(stored.token_hash).not.toBe(result.body.token);
  });

  test("duplicate email returns the same neutral shape with no token", async () => {
    const { db } = createDb();
    await call(db, { email: "a@x.io" });
    const duplicate = await call(db, { email: "a@x.io" });
    expect(duplicate.status).toBe(200);
    expect(duplicate.body).toEqual({ ok: true });
  });

  test("invalid email is rejected", async () => {
    const { db } = createDb();
    const result = await call(db, { email: "bukan-email" });
    expect(result.status).toBe(422);
    expect(result.body.error).toBe("invalid_email");
  });

  test("malformed JSON is rejected", async () => {
    const { db } = createDb();
    const result = await call(db, "not json");
    expect(result.status).toBe(400);
  });

  test("JSON null and arrays are rejected instead of crashing", async () => {
    const { db } = createDb();
    expect((await call(db, "null")).status).toBe(400);
    expect((await call(db, "[]")).status).toBe(400);
  });

  test("non-JSON content type is rejected with 415", async () => {
    const { db, rows } = createDb();
    const result = await call(db, { email: "a@x.io" }, { contentType: "text/plain" });
    expect(result.status).toBe(415);
    expect(rows.size).toBe(0);
  });

  test("oversized body is rejected before parsing", async () => {
    const { db, rows } = createDb();
    const result = await call(db, { email: "a@x.io", source: "x".repeat(5000) });
    expect(result.status).toBe(400);
    expect(rows.size).toBe(0);
  });

  test("unknown source is stored as null", async () => {
    const { db, rows } = createDb();
    await call(db, { email: "a@x.io", source: "<script>" });
    expect(rows.get("a@x.io")!.source).toBeNull();
  });

  test("honeypot filled returns ok without storing", async () => {
    const { db, rows } = createDb();
    const result = await call(db, { email: "bot@x.io", website: "spam" });
    expect(result.body).toEqual({ ok: true });
    expect(rows.size).toBe(0);
  });
});

describe("profile", () => {
  test("correct token updates platform and usage", async () => {
    const { db, rows } = createDb();
    const joined = await call(db, { email: "a@x.io" });
    const result = await call(db, { email: "a@x.io", token: joined.body.token, profile: true, ...PROFILE });
    expect(result.status).toBe(200);
    expect(rows.get("a@x.io")!.platform).toBe(PROFILE.platform);
    expect(rows.get("a@x.io")!.usage).toBe(PROFILE.usage);
  });

  test("wrong token is forbidden and changes nothing", async () => {
    const { db, rows } = createDb();
    await call(db, { email: "a@x.io" });
    const result = await call(db, { email: "a@x.io", token: "forged", profile: true, ...PROFILE });
    expect(result.status).toBe(403);
    expect(rows.get("a@x.io")!.platform).toBeUndefined();
  });

  test("existing email cannot be claimed without its original token", async () => {
    const { db, rows } = createDb();
    await call(db, { email: "victim@x.io" });
    const attack = await call(db, { email: "victim@x.io", profile: true, ...PROFILE });
    expect(attack.status).toBe(422);
    expect(rows.get("victim@x.io")!.platform).toBeUndefined();
  });

  test("missing token is rejected", async () => {
    const { db } = createDb();
    const result = await call(db, { email: "a@x.io", profile: true, ...PROFILE });
    expect(result.status).toBe(422);
  });

  test("values outside the allowlist are rejected", async () => {
    const { db } = createDb();
    const joined = await call(db, { email: "a@x.io" });
    const badPlatform = await call(db, { email: "a@x.io", token: joined.body.token, profile: true, platform: "Linux", usage: "Jarang" });
    const badUsage = await call(db, { email: "a@x.io", token: joined.body.token, profile: true, platform: PROFILE.platform, usage: "<script>" });
    expect(badPlatform.status).toBe(422);
    expect(badUsage.status).toBe(422);
  });
});

describe("rate limit", () => {
  test("requests above the limit from one IP are rejected with 429", async () => {
    const { db } = createDb();
    let last = { status: 0, body: {} as Record<string, unknown> };
    for (let i = 0; i < 11; i++) {
      last = await call(db, { email: `user${i}@x.io` }, { ip: "198.51.100.9" });
    }
    expect(last.status).toBe(429);
    expect(last.body.error).toBe("rate_limited");
  });

  test("another IP is not affected by one IP's usage", async () => {
    const { db } = createDb();
    for (let i = 0; i < 11; i++) {
      await call(db, { email: `spam${i}@x.io` }, { ip: "198.51.100.10" });
    }
    const other = await call(db, { email: "fresh@x.io" }, { ip: "198.51.100.11" });
    expect(other.status).toBe(200);
  });
});

describe("domain allowlist & form fallback", () => {
  test("24H2 platform option is accepted end-to-end", async () => {
    const { db, rows } = createDb();
    expect(WAITLIST_PLATFORMS).toContain("Windows 11 24H2 atau lebih baru");
    const joined = await call(db, { email: "win24@x.io" });
    const result = await call(db, {
      email: "win24@x.io",
      token: joined.body.token,
      profile: true,
      platform: "Windows 11 24H2 atau lebih baru",
      usage: WAITLIST_USAGES[2],
    });
    expect(result.status).toBe(200);
    expect(rows.get("win24@x.io")!.platform).toBe("Windows 11 24H2 atau lebih baru");
  });

  test("urlencoded form POST saves email and redirects with status", async () => {
    const { db, rows } = createDb();
    const request = new Request("https://example.test/api/waitlist", {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "cf-connecting-ip": "203.0.113.88",
      },
      body: new URLSearchParams({ email: "Fallback@Example.com" }).toString(),
    });
    const response = await onRequestPost({ request, env: { DB: db } } as never);
    expect(response.status).toBe(303);
    expect(response.headers.get("location")).toBe("https://example.test/?waitlist=ok#daftar");
    expect(rows.has("fallback@example.com")).toBe(true);
  });

  test("urlencoded form POST rejects bad email via redirect without saving", async () => {
    const { db, rows } = createDb();
    const request = new Request("https://example.test/api/waitlist", {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "cf-connecting-ip": "203.0.113.89",
      },
      body: new URLSearchParams({ email: "invalid-addr" }).toString(),
    });
    const response = await onRequestPost({ request, env: { DB: db } } as never);
    expect(response.status).toBe(303);
    expect(response.headers.get("location")).toBe("https://example.test/?waitlist=invalid#daftar");
    expect(rows.size).toBe(0);
  });
});

describe("D1 migration schema compatibility", () => {
  test("0001_init.sql executes all waitlist and rate_limits queries on SQLite", async () => {
    const sqlite = new Database(":memory:");
    const sqlPath = resolve(import.meta.dir, "..", "migrations", "0001_init.sql");
    sqlite.exec(readFileSync(sqlPath, "utf8"));

    const db = {
      prepare(sql: string) {
        let params: unknown[] = [];
        const stmt = {
          bind(...values: unknown[]) {
            params = values;
            return stmt;
          },
          async first<T>() {
            return (sqlite.query(sql).get(...(params as never[])) as T | undefined) ?? null;
          },
          async run() {
            const info = sqlite.query(sql).run(...(params as never[]));
            return { meta: { changes: info.changes } };
          },
        };
        return stmt;
      },
    } as unknown as D1Database;

    const joined = await call(db, { email: "sql@example.com", source: "hero" });
    expect(joined.status).toBe(200);
    expect(typeof joined.body.token).toBe("string");

    const updated = await call(db, {
      email: "sql@example.com",
      token: joined.body.token,
      profile: true,
      platform: "Windows 11 24H2 atau lebih baru",
      usage: "Hampir setiap hari",
    });
    expect(updated.status).toBe(200);

    const row = sqlite
      .query("SELECT email, source, platform, usage FROM waitlist WHERE email = ?")
      .get("sql@example.com") as {
      email: string;
      source: string;
      platform: string;
      usage: string;
    };
    expect(row.source).toBe("hero");
    expect(row.platform).toBe("Windows 11 24H2 atau lebih baru");
    expect(row.usage).toBe("Hampir setiap hari");
    sqlite.close();
  });
});
