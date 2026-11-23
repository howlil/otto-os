type ReleaseAsset = {
  name: string;
  browser_download_url: string;
  download_count: number;
};

type GitHubRelease = {
  tag_name: string;
  draft: boolean;
  prerelease: boolean;
  created_at: string;
  assets: ReleaseAsset[];
};

const RELEASES_URL = "https://api.github.com/repos/howlil/otto-os/releases?per_page=100";
const INSTALLER_NAME = /^otto-os-setup-\d+(?:\.\d+)*-x64\.exe$/i;
const PORTABLE_ZIP_NAME = /^otto-os-\d+(?:\.\d+)*(?:-[0-9A-Za-z.-]+)?-windows-x64\.zip$/i;
const INSTALLER_URL = "https://github.com/howlil/otto-os/releases/download/";
const CACHE_SECONDS = 300;
const REQUEST_TIMEOUT_MS = 8000;
const MAX_RELEASE_PAGES = 20;

export function isCountedReleaseAsset(name: string): boolean {
  return INSTALLER_NAME.test(name) || PORTABLE_ZIP_NAME.test(name);
}

export type ReleaseSummary = {
  version: string | null;
  downloadUrl: string | null;
  downloads: number;
  prerelease: boolean;
};

export function summarizeReleases(releases: GitHubRelease[]): ReleaseSummary {
  const publishedReleases = releases
    .filter((release) => !release.draft)
    .sort((left, right) => {
      if (left.prerelease !== right.prerelease) {
        return left.prerelease ? 1 : -1;
      }
      return Date.parse(right.created_at) - Date.parse(left.created_at);
    });
  const installerFor = (release: GitHubRelease) =>
    release.assets.find((asset) => INSTALLER_NAME.test(asset.name));
  const latest = publishedReleases.find((release) => {
    const installer = installerFor(release);
    return installer?.browser_download_url.startsWith(INSTALLER_URL);
  });
  const latestInstaller = latest ? installerFor(latest) : undefined;
  const downloads = publishedReleases.reduce(
    (total, release) =>
      total +
      release.assets
        .filter((asset) => isCountedReleaseAsset(asset.name))
        .reduce((assetTotal, asset) => assetTotal + asset.download_count, 0),
    0,
  );
  return {
    version: latest && latestInstaller ? latest.tag_name : null,
    downloadUrl: latestInstaller?.browser_download_url ?? null,
    downloads,
    prerelease: latest && latestInstaller ? latest.prerelease : false,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isAsset(value: unknown): value is ReleaseAsset {
  return (
    isRecord(value) &&
    typeof value.name === "string" &&
    typeof value.browser_download_url === "string" &&
    Number.isInteger(value.download_count) &&
    Number(value.download_count) >= 0
  );
}

function isRelease(value: unknown): value is GitHubRelease {
  return (
    isRecord(value) &&
    typeof value.tag_name === "string" &&
    typeof value.draft === "boolean" &&
    typeof value.prerelease === "boolean" &&
    typeof value.created_at === "string" &&
    Number.isFinite(Date.parse(value.created_at)) &&
    Array.isArray(value.assets) &&
    value.assets.every(isAsset)
  );
}

function nextReleasePage(response: Response): string | null {
  const nextLink = response.headers
    .get("link")
    ?.split(",")
    .find((link) => /\brel="next"/.test(link));
  const url = nextLink?.match(/<([^>]+)>/)?.[1];
  if (!url) return null;

  const next = new URL(url);
  return next.origin === "https://api.github.com" && next.pathname === "/repos/howlil/otto-os/releases"
    ? next.toString()
    : null;
}

function json(body: Record<string, unknown>, status = 200, cache = false): Response {
  return Response.json(body, {
    status,
    headers: {
      "cache-control": cache ? `public, max-age=60, s-maxage=${CACHE_SECONDS}` : "no-store",
    },
  });
}

export type ReleaseSummaryResult =
  | { ok: true; summary: ReleaseSummary }
  | { ok: false; status: number; error: string };

export async function fetchReleaseSummary(requestUrl: string): Promise<ReleaseSummaryResult> {
  const cacheKey = new Request(new URL("/api/beta-downloads", requestUrl));
  const cache = (caches as CacheStorage & { default: Cache }).default;
  const cached = await cache.match(cacheKey).catch(() => undefined);
  if (cached) {
    try {
      const payload = (await cached.clone().json()) as unknown;
      if (
        isRecord(payload) &&
        Number.isInteger(payload.downloads) &&
        Number(payload.downloads) >= 0
      ) {
        return {
          ok: true,
          summary: {
            version: typeof payload.version === "string" ? payload.version : null,
            downloadUrl: typeof payload.downloadUrl === "string" ? payload.downloadUrl : null,
            downloads: Number(payload.downloads),
            prerelease: payload.prerelease === true,
          },
        };
      }
    } catch {
    }
  }

  try {
    const releases: GitHubRelease[] = [];
    const signal = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
    let nextPage: string | null = RELEASES_URL;
    let pagesRead = 0;

    while (nextPage && pagesRead < MAX_RELEASE_PAGES) {
      const response = await fetch(nextPage, {
        headers: {
          Accept: "application/vnd.github+json",
          "User-Agent": "OttoOS-Landing",
        },
        signal,
      });
      if (!response.ok) return { ok: false, status: 503, error: "release_metadata_unavailable" };

      const payload: unknown = await response.json();
      if (!Array.isArray(payload)) return { ok: false, status: 502, error: "invalid_release_metadata" };

      releases.push(...payload.filter(isRelease));
      nextPage = nextReleasePage(response);
      pagesRead += 1;
    }
    if (nextPage) return { ok: false, status: 502, error: "release_metadata_incomplete" };

    const summary = summarizeReleases(releases);
    const result = json(summary, 200, true);
    await cache.put(cacheKey, result.clone()).catch(() => undefined);
    return { ok: true, summary };
  } catch {
    return { ok: false, status: 503, error: "release_metadata_unavailable" };
  }
}

export const onRequestGet: PagesFunction = async ({ request }) => {
  const outcome = await fetchReleaseSummary(request.url);
  if (!outcome.ok) {
    return json({ error: outcome.error }, outcome.status);
  }
  return json(outcome.summary, 200, true);
};
