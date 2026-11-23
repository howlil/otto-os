import { fetchReleaseSummary } from "./beta-downloads";

const CACHE_SECONDS = 300;
const LABEL = "downloads";
const LABEL_WIDTH = 67;

export function formatBadgeCount(count: number | null): string {
  if (count === null || !Number.isFinite(count) || count < 0) return "—";
  if (count >= 10_000) {
    return `${(count / 1_000).toFixed(1).replace(/\.0$/, "")}k`;
  }
  return new Intl.NumberFormat("en-US").format(Math.trunc(count));
}

function escapeXml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function renderDownloadsBadgeSvg(count: number | null): string {
  const value = formatBadgeCount(count);
  const valueWidth = Math.max(25, value.length * 7 + 12);
  const totalWidth = LABEL_WIDTH + valueWidth;
  const labelCenter = Math.round((LABEL_WIDTH / 2) * 10);
  const valueCenter = Math.round((LABEL_WIDTH + valueWidth / 2) * 10);
  const labelTextLen = (LABEL_WIDTH - 10) * 10;
  const valueTextLen = (valueWidth - 10) * 10;
  const safeValue = escapeXml(value);
  const title = `${LABEL}: ${safeValue}`;

  return [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${totalWidth}" height="20" role="img" aria-label="${title}">`,
    `<title>${title}</title>`,
    `<g shape-rendering="crispEdges">`,
    `<rect width="${LABEL_WIDTH}" height="20" fill="#555"/>`,
    `<rect x="${LABEL_WIDTH}" width="${valueWidth}" height="20" fill="#1f2328"/>`,
    `</g>`,
    `<g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">`,
    `<text x="${labelCenter}" y="140" transform="scale(.1)" fill="#fff" textLength="${labelTextLen}">${LABEL}</text>`,
    `<text x="${valueCenter}" y="140" transform="scale(.1)" fill="#fff" textLength="${valueTextLen}">${safeValue}</text>`,
    `</g>`,
    `</svg>`,
  ].join("");
}

export const onRequestGet: PagesFunction = async ({ request }) => {
  const outcome = await fetchReleaseSummary(request.url);
  const count = outcome.ok ? outcome.summary.downloads : null;
  const svg = renderDownloadsBadgeSvg(count);
  return new Response(svg, {
    status: 200,
    headers: {
      "content-type": "image/svg+xml; charset=utf-8",
      "cache-control": outcome.ok
        ? `public, max-age=60, s-maxage=${CACHE_SECONDS}`
        : "no-store",
    },
  });
};
