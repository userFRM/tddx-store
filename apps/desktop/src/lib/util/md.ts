// Tiny markdown renderer for yaml-sourced descriptions. ThetaData's
// openapiv3.yaml ships description fields written in markdown
// (bold, italic, code, links, lists). `marked` parses + emits HTML;
// we render via `{@html}` in Svelte. Inputs come from the upstream
// yaml only — no user-controlled content reaches `renderMarkdown`,
// so XSS surface is limited to whatever ThetaData publishes in
// their docs.
import { marked } from "marked";
import { openUrl } from "@tauri-apps/plugin-opener";

marked.setOptions({ gfm: true, breaks: true });

/** ThetaData's openapi yaml stores doc-cross-refs as bare relative paths
 *  (`Articles/Data-And-Requests/OHLC-EOD.html`). Inside the Tauri
 *  webview those resolve against `http://localhost:1420/...` and 404
 *  inside the SPA. Anchor renderer rewrites every non-absolute,
 *  non-hash href against this base before emitting. The base matches
 *  the canonical docs site so links land on the right page. */
const DOC_BASE = "https://docs.thetadata.us/";

function absolutize(href: string): string {
  if (!href) return href;
  if (href.startsWith("#")) return href;
  if (href.startsWith("javascript:") || href.startsWith("mailto:")) return href;
  if (/^https?:\/\//i.test(href)) return href;
  // Strip a leading "/" so we don't double-slash against DOC_BASE's
  // trailing "/". Anything else (relative, ./, ../) is appended as-is;
  // the browser's URL normalizer handles `../` traversal.
  return DOC_BASE + href.replace(/^\/+/, "");
}

// Anchor renderer marks every link as external so the global click
// delegate (installed below) can intercept and hand the URL to the
// OS browser via tauri-plugin-opener. Without this, marked anchors
// navigate the Tauri webview itself — the embedded webview loads
// the asset:// scheme, has no route for https URLs, and renders a
// 404 in-app; the back button then re-mounts the SPA which resets
// `connState` to "idle" and re-shows the LoginGate.
marked.use({
  renderer: {
    link({ href, title, tokens }) {
      const text = this.parser.parseInline(tokens);
      const t = title ? ` title="${title}"` : "";
      const resolved = absolutize(href);
      return `<a href="${resolved}"${t} target="_blank" rel="noopener noreferrer" data-external="1">${text}</a>`;
    },
  },
});

/** ThetaData yaml descriptions occasionally use 3+ consecutive
 *  asterisks (`****Important****`) which CommonMark parses as a
 *  bold-open + literal-`**` + close (visible junk `**` chars).
 *  Collapse runs of 3+ asterisks down to 2 before parsing so the
 *  intended bold renders cleanly. */
function sanitize(src: string): string {
  return src.replace(/\*{3,}/g, "**");
}

export function renderMarkdown(src: string | null | undefined): string {
  if (!src) return "";
  try {
    return marked.parse(sanitize(src)) as string;
  } catch {
    return src;
  }
}

/** Install the global click delegate once on app boot. Catches every
 *  `<a data-external="1">` produced by `renderMarkdown` (and any plain
 *  `target="_blank"` anchor) and routes the href through the OS
 *  default browser instead of letting the Tauri webview try to load
 *  the URL — which would 404 inside the asset:// shell and trash SPA
 *  state. Safe to call multiple times; the flag prevents duplicate
 *  listeners on HMR. */
let installed = false;
export function installMarkdownLinkInterceptor() {
  if (installed || typeof document === "undefined") return;
  installed = true;
  document.addEventListener("click", (e) => {
    const target = e.target as HTMLElement | null;
    const a = target?.closest?.("a") as HTMLAnchorElement | null;
    if (!a) return;
    const href = a.getAttribute("href");
    if (!href) return;
    // Internal hash links + javascript: URLs stay on the renderer.
    if (href.startsWith("#") || href.startsWith("javascript:")) return;
    // Resolve relative paths against the ThetaData docs base — same
    // logic the marked renderer applies, but defensively here too in
    // case yaml descriptions ever ship raw `<a>` tags that bypass
    // the markdown link grammar.
    const resolved = absolutize(href);
    if (!/^https?:\/\//i.test(resolved) && !resolved.startsWith("mailto:")) return;
    e.preventDefault();
    void openUrl(resolved).catch(() => {});
  });
}
