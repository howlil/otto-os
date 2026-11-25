import { describe, expect, test } from "bun:test";

import { isCountedReleaseAsset, summarizeReleases } from "../functions/api/beta-downloads";
import { formatBadgeCount, renderDownloadsBadgeSvg } from "../functions/api/downloads-badge";

describe("beta-downloads release counting", () => {
  test("counts installer executables and portable zips while ignoring checksums and WinGet manifests", () => {
    expect(isCountedReleaseAsset("otto-os-setup-1.3.1-x64.exe")).toBe(true);
    expect(isCountedReleaseAsset("otto-os-1.3.1-windows-x64.zip")).toBe(true);
    expect(isCountedReleaseAsset("otto-os-1.4.0-beta.1-windows-x64.zip")).toBe(true);
    expect(isCountedReleaseAsset("SHA256SUMS.txt")).toBe(false);
    expect(isCountedReleaseAsset("howlil.OttoOS.yaml")).toBe(false);
    expect(isCountedReleaseAsset("howlil.OttoOS.installer.yaml")).toBe(false);
    expect(isCountedReleaseAsset("howlil.OttoOS.locale.en-US.yaml")).toBe(false);
  });

  test("sums installer and portable zip downloads across published releases and targets latest installer", () => {
    const summary = summarizeReleases([
      {
        tag_name: "v1.3.1",
        draft: false,
        prerelease: false,
        created_at: "2026-11-23T15:00:00Z",
        assets: [
          {
            name: "otto-os-setup-1.3.1-x64.exe",
            browser_download_url:
              "https://github.com/howlil/otto-os/releases/download/v1.3.1/otto-os-setup-1.3.1-x64.exe",
            download_count: 5,
          },
          {
            name: "otto-os-1.3.1-windows-x64.zip",
            browser_download_url:
              "https://github.com/howlil/otto-os/releases/download/v1.3.1/otto-os-1.3.1-windows-x64.zip",
            download_count: 1,
          },
          {
            name: "SHA256SUMS.txt",
            browser_download_url: "https://github.com/howlil/otto-os/releases/download/v1.3.1/SHA256SUMS.txt",
            download_count: 20,
          },
          {
            name: "howlil.OttoOS.yaml",
            browser_download_url: "https://github.com/howlil/otto-os/releases/download/v1.3.1/howlil.OttoOS.yaml",
            download_count: 10,
          },
        ],
      },
      {
        tag_name: "v1.3.0",
        draft: false,
        prerelease: false,
        created_at: "2026-11-23T05:00:00Z",
        assets: [
          {
            name: "otto-os-setup-1.3.0-x64.exe",
            browser_download_url:
              "https://github.com/howlil/otto-os/releases/download/v1.3.0/otto-os-setup-1.3.0-x64.exe",
            download_count: 33,
          },
          {
            name: "otto-os-1.3.0-windows-x64.zip",
            browser_download_url:
              "https://github.com/howlil/otto-os/releases/download/v1.3.0/otto-os-1.3.0-windows-x64.zip",
            download_count: 3,
          },
        ],
      },
      {
        tag_name: "v1.4.0-draft",
        draft: true,
        prerelease: false,
        created_at: "2026-11-24T00:00:00Z",
        assets: [
          {
            name: "otto-os-setup-1.4.0-x64.exe",
            browser_download_url:
              "https://github.com/howlil/otto-os/releases/download/v1.4.0-draft/otto-os-setup-1.4.0-x64.exe",
            download_count: 99,
          },
        ],
      },
    ]);

    expect(summary).toEqual({
      version: "v1.3.1",
      downloadUrl: "https://github.com/howlil/otto-os/releases/download/v1.3.1/otto-os-setup-1.3.1-x64.exe",
      downloads: 42,
      prerelease: false,
    });
  });

  test("renders flat-square SVG badge matching landing downloads count", () => {
    expect(formatBadgeCount(55)).toBe("55");
    expect(formatBadgeCount(1250)).toBe("1,250");
    expect(formatBadgeCount(12_400)).toBe("12.4k");
    expect(formatBadgeCount(null)).toBe("—");

    const svg = renderDownloadsBadgeSvg(55);
    expect(svg).toContain('height="20"');
    expect(svg).toContain("<title>downloads: 55</title>");
    expect(svg).toContain(">55</text>");
  });
});
