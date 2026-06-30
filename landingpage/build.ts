// BitModel landing page build.
//
// Uses the shared @modelmirror/ui design system (same tokens, components, and
// copy-gate as modelmirror.org) and emits a self-contained static site to dist/.
// Run inside the Modelmirror workspace, where ../../shared and ../../web exist:
//
//   bun run landingpage/build.ts        # from the Bitmodel repo root
//
// The committed dist/ is the deployable artifact; it needs none of the above.

import { mkdir } from "node:fs/promises";
import { readVersion } from "../../shared/ui/version";
import { icon, iconNames, type IconName } from "../../shared/ui/icons/icons";
import { logoSvg } from "../../shared/ui/brand/logo";
import { copyFontsTo } from "../../shared/ui/fonts";
import { checkCopy } from "../../web/check-copy";

const here = new URL("./", import.meta.url);
const uiDist = new URL("../../shared/ui/dist/", here);
const dist = new URL("dist/", here);

const PAGES = ["index.html"];
const GITHUB_URL = "https://github.com/AutismLLM/Bitmodel";
const DOWNLOAD_URL = "https://github.com/AutismLLM/Bitmodel/releases/latest";
const WHITEPAPER_URL = "./Modelmirror-Whitepaper.pdf";
const WHITEPAPER_SRC = new URL("../../../openmodel-mirror-spec/Modelmirror-Whitepaper.pdf", here);

// Forbidden-words gate (the exact same checker the main site uses), before writing.
// Paths are relative to the Modelmirror workspace root (where check-copy resolves).
const violations = await checkCopy(PAGES.map((p) => `Bitmodel/landingpage/${p}`));
if (violations.length > 0) {
  console.error(`copy-gate: ${violations.length} violation(s) in landing copy:`);
  for (const v of violations) console.error(`  ${v.file}:${v.line}  "${v.token}"  | ${v.snippet}`);
  throw new Error("copy-gate failed: banned word, internal term, or banned char");
}

const version = await readVersion();

const uiCss = await Bun.file(new URL("ui.css", uiDist)).text();
const siteCss = await Bun.file(new URL("styles/site.css", here)).text();
const stylesCss = `${uiCss}\n${siteCss}`;
const uiJs = await Bun.file(new URL("ui.js", uiDist)).text();
const noFlash = await Bun.file(new URL("no-flash.js", uiDist)).text();

const hasher = new Bun.CryptoHasher("sha256");
hasher.update(stylesCss);
hasher.update(uiJs);
const buildHash = hasher.digest("hex").slice(0, 12);

function renderPage(src: string): string {
  return src
    .replace(/\{\{icon:([a-z-]+)(?::(\d+))?\}\}/g, (_m, name: string, size?: string) => {
      if (!(iconNames as readonly string[]).includes(name)) throw new Error(`unknown icon: ${name}`);
      return icon(name as IconName, size ? { size: Number(size) } : {});
    })
    .replace(/\{\{logo(?::(\d+))?\}\}/g, (_m, size?: string) =>
      logoSvg({ size: size ? Number(size) : 28, className: "site-brand__logo", title: "" }),
    )
    .replaceAll("{{UI_VERSION}}", version)
    .replaceAll("{{BUILD_HASH}}", buildHash)
    .replaceAll("{{GITHUB_URL}}", GITHUB_URL)
    .replaceAll("{{DOWNLOAD_URL}}", DOWNLOAD_URL)
    .replaceAll("{{WHITEPAPER_URL}}", WHITEPAPER_URL)
    .replace("{{NO_FLASH}}", noFlash.trim());
}

await mkdir(dist.pathname, { recursive: true });
await copyFontsTo(dist);
await Bun.write(new URL("styles.css", dist), stylesCss);
await Bun.write(new URL("ui.js", dist), uiJs);
await Bun.write(new URL("favicon.svg", dist), `${logoSvg({ size: 32, title: "BitModel" })}\n`);
for (const page of PAGES) {
  const src = await Bun.file(new URL(page, here)).text();
  await Bun.write(new URL(page, dist), renderPage(src));
}

// Host the whitepaper next to the page so the link is self-contained.
const wp = Bun.file(WHITEPAPER_SRC);
if (await wp.exists()) {
  await Bun.write(new URL("Modelmirror-Whitepaper.pdf", dist), wp);
} else {
  console.warn(`warning: whitepaper not found at ${WHITEPAPER_SRC.pathname}; link will 404`);
}

console.log(`landing built with @modelmirror/ui@${version} (build ${buildHash}) -> landingpage/dist/`);
