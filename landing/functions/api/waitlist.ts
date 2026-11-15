import {
  EMAIL_PATTERN,
  MAX_EMAIL_LENGTH,
  WAITLIST_PLATFORMS,
  WAITLIST_SOURCES,
  WAITLIST_USAGES,
} from "../../src/domain/waitlist";

interface Env {
  DB: D1Database;
}

const MAX_BODY_BYTES = 4096;
const MAX_TOKEN_LENGTH = 64;
const RATE_WINDOW_MS = 10 * 60 * 1000;
const RATE_LIMIT = 10;

const PLATFORMS = new Set<string>(WAITLIST_PLATFORMS);
const USAGES = new Set<string>(WAITLIST_USAGES);
const SOURCES = new Set<string>(WAITLIST_SOURCES);

function json(body: Record<string, unknown>, status = 200): Response {
  return Response.json(body, { status, headers: { "cache-control": "no-store" } });
}

function redirectToForm(request: Request, status: string): Response {
  const target = new URL("/", request.url);
  target.searchParams.set("waitlist", status);
  target.hash = "daftar";
  return Response.redirect(target.toString(), 303);
}

function clean(value: unknown, max: number): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed ? trimmed.slice(0, max) : null;
}

async function sha256Hex(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function exceedsRateLimit(db: D1Database, ip: string): Promise<boolean> {
  const bucket = Math.floor(Date.now() / RATE_WINDOW_MS);
  const row = await db
    .prepare(
      "INSERT INTO rate_limits (key, bucket, count) VALUES (?, ?, 1) ON CONFLICT(key) DO UPDATE SET count = count + 1 RETURNING count",
    )
    .bind(`${ip}:${bucket}`, bucket)
    .first<{ count: number }>();
  if (row?.count === 1) {
    await db.prepare("DELETE FROM rate_limits WHERE bucket < ?").bind(bucket).run();
  }
  return (row?.count ?? 0) > RATE_LIMIT;
}

async function readPayload(
  request: Request,
  isForm: boolean,
): Promise<Record<string, unknown> | null> {
  const text = await request.text();
  if (new TextEncoder().encode(text).byteLength > MAX_BODY_BYTES) {
    return null;
  }
  if (isForm) {
    const params = new URLSearchParams(text);
    return Object.fromEntries(params.entries());
  }
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return null;
    }
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}

export const onRequestPost: PagesFunction<Env> = async ({ request, env }) => {
  const contentType = (request.headers.get("content-type") ?? "").toLowerCase();
  const isJson = contentType.startsWith("application/json");
  const isForm = contentType.startsWith("application/x-www-form-urlencoded");
  if (!isJson && !isForm) {
    return json({ ok: false, error: "unsupported_media_type" }, 415);
  }

  const payload = await readPayload(request, isForm);
  if (!payload) {
    return isForm
      ? redirectToForm(request, "error")
      : json({ ok: false, error: "invalid_body" }, 400);
  }

  if (clean(payload.website, 200)) {
    return isForm ? redirectToForm(request, "ok") : json({ ok: true });
  }

  const ip = request.headers.get("cf-connecting-ip") ?? "unknown";
  if (await exceedsRateLimit(env.DB, ip)) {
    return isForm
      ? redirectToForm(request, "rate_limited")
      : json({ ok: false, error: "rate_limited" }, 429);
  }

  const email = clean(payload.email, MAX_EMAIL_LENGTH)?.toLowerCase() ?? "";
  if (!email || email.length > MAX_EMAIL_LENGTH || !EMAIL_PATTERN.test(email)) {
    return isForm
      ? redirectToForm(request, "invalid")
      : json({ ok: false, error: "invalid_email" }, 422);
  }

  if (payload.profile === true) {
    const token = clean(payload.token, MAX_TOKEN_LENGTH);
    const platform = clean(payload.platform, 60);
    const usage = clean(payload.usage, 60);
    if (!token || !platform || !usage || !PLATFORMS.has(platform) || !USAGES.has(usage)) {
      return json({ ok: false, error: "invalid_profile" }, 422);
    }
    const updated = await env.DB.prepare(
      "UPDATE waitlist SET platform = ?, usage = ?, updated_at = datetime('now') WHERE email = ? AND token_hash = ?",
    )
      .bind(platform, usage, email, await sha256Hex(token))
      .run();
    if (updated.meta.changes === 0) {
      return json({ ok: false, error: "forbidden" }, 403);
    }
    return json({ ok: true });
  }

  const source = clean(payload.source, 20) ?? (isForm ? "hero" : null);
  const token = crypto.randomUUID();
  const result = await env.DB.prepare(
    "INSERT OR IGNORE INTO waitlist (email, source, token_hash) VALUES (?, ?, ?)",
  )
    .bind(email, source && SOURCES.has(source) ? source : null, await sha256Hex(token))
    .run();

  if (result.meta.changes === 0) {
    return isForm ? redirectToForm(request, "ok") : json({ ok: true });
  }
  return isForm ? redirectToForm(request, "ok") : json({ ok: true, token });
};
