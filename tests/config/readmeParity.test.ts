import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

// 本 fork 的中英主页介绍 Web 服务；其余语言保存上游桌面文档并链接到 Web 指南。
const ROOT = path.resolve(__dirname, "../..");

const READMES = [
  ["README_EN.md", "English"],
  ["README_ZH.md", "简体中文"],
  ["README_ZH-TW.md", "繁體中文"],
  ["README_JA.md", "日本語"],
  ["README_KO.md", "한국어"],
  ["README_TR.md", "Türkçe"],
  ["README_RU.md", "Русский"],
  ["README_DE.md", "Deutsch"],
] as const;

// 简体中文版面向大陆读者，赞助商表有地区版本（国内活动页、中文横幅），
// 只校验行数；其余各版与英文版逐个链接对齐。
const REGIONAL_SPONSOR_VARIANTS = new Set(["README_ZH.md"]);

const read = (file: string) => readFileSync(path.join(ROOT, file), "utf8");

const headingLevels = (text: string) =>
  [...text.matchAll(/^(#{1,4}) /gm)].map((m) => m[1].length);

const sponsorSection = (text: string) => {
  const start = text.search(/^## /m);
  const next = text.slice(start + 3).search(/^## /m);
  return text.slice(start, start + 3 + next);
};

const htmlLinks = (text: string) =>
  [...text.matchAll(/(?:href|src)="([^"]+)"/g)].map((m) => m[1]);

const sponsorRows = (text: string) =>
  (sponsorSection(text).match(/<tr>/g) ?? []).length;

const desktopReference = read("README_DE.md");
const DESKTOP_READMES = READMES.filter(([file]) => file !== "README_EN.md");

describe.each(DESKTOP_READMES)("%s", (file) => {
  it("exists", () => {
    expect(existsSync(path.join(ROOT, file))).toBe(true);
  });

  const text = existsSync(path.join(ROOT, file)) ? read(file) : "";

  it("links every other README from its language switcher", () => {
    const switcher = text
      .split("\n")
      .find(
        (line) => line.includes("(CHANGELOG.md)") && line.includes("README"),
      );
    expect(switcher, "language switcher line").toBeDefined();
    for (const [other, name] of READMES) {
      if (other === file) {
        expect(switcher).toContain(name);
        expect(switcher).not.toContain(`(${other})`);
      } else {
        expect(switcher).toContain(`[${name}](${other})`);
      }
    }
  });

  it("keeps the same heading structure as the desktop reference", () => {
    expect(headingLevels(text)).toEqual(headingLevels(desktopReference));
  });

  it("keeps the same number of sponsor rows as the desktop reference", () => {
    expect(sponsorRows(text)).toBe(sponsorRows(desktopReference));
  });

  if (!REGIONAL_SPONSOR_VARIANTS.has(file)) {
    it("keeps every HTML link and image in the same order as the desktop reference", () => {
      expect(htmlLinks(text)).toEqual(htmlLinks(desktopReference));
    });
  }
});

describe("fork README destinations", () => {
  it.each(["README.md", "README_EN.md"])(
    "%s describes the Web server",
    (file) => {
      const text = read(file);
      expect(text).toContain("cc-switch-server");
      expect(text).toContain("docs/web-mode-zh.md");
      expect(text).toContain("zhangzhiwei-zzw/cc-switch-headless");
    },
  );
  it.each(DESKTOP_READMES)("%s directs Web users to the fork guide", (file) => {
    const banner = read(file).split("\n")[0];
    expect(banner).toContain("README_EN.md");
    expect(banner).toContain("docs/README.md");
  });
});
