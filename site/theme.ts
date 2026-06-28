// Visual theme: terminal/CLI-styled page shell. Isolated from build logic.

export const SITE = {
  name: "llmff",
  tagline: "FFmpeg-shaped pipelines for LLM workflows",
  repo: "https://github.com/syndicalt/llmff",
  base: process.env.BASE_URL || "/",
};

export interface TocEntry { level: number; text: string; slug: string; }

export interface ShellOpts {
  title: string;
  description?: string;
  bodyClass?: string;
  /** rendered sidebar HTML (doc layout); "" → full-width page */
  sidebar?: string;
  /** rendered right-rail TOC HTML or "" */
  toc?: string;
  /** main content HTML */
  content: string;
  /** active top section id */
  section?: string;
  /** prev/next nav HTML */
  pager?: string;
  /** breadcrumb path shown in the window title bar, e.g. "docs/quickstart" */
  crumb?: string;
}

const B = SITE.base.replace(/\/$/, "");
export const url = (p: string): string => B + (p.startsWith("/") ? p : "/" + p);

const TABS: ReadonlyArray<readonly [string, string, string]> = [
  ["docs", "docs", "/docs/quickstart.html"],
  ["blog", "blog", "/blog/index.html"],
  ["research", "research", "/research.html"],
  ["releases", "releases", "/releases.html"],
];

export function headerHtml(section?: string, crumb?: string): string {
  const tabs = TABS.map(
    ([id, label, href]) =>
      `<a class="tab${section === id ? " active" : ""}" href="${url(href)}">${label}</a>`,
  ).join("");
  const path = crumb ? `<span class="tb-path">~/llmff/<b>${escapeHtml(crumb)}</b></span>` : `<span class="tb-path">~/llmff</span>`;
  return `<header class="term-bar">
  <div class="tb-inner">
    <button id="menu-toggle" class="icon-btn menu-toggle" aria-label="Menu">
      <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path fill="currentColor" d="M3 6h18v2H3zM3 11h18v2H3zM3 16h18v2H3z"/></svg>
    </button>
    <a class="brand" href="${url("/index.html")}">
      <span class="dots"><i class="dot r"></i><i class="dot y"></i><i class="dot g"></i></span>
      <span class="brand-name">llmff</span>
    </a>
    ${path}
    <nav class="tabs">${tabs}</nav>
    <div class="tb-right">
      <button id="search-open" class="search-btn" aria-label="Search">
        <span class="prompt">/</span><span>search</span>
      </button>
      <a class="tab ghost" href="${SITE.repo}" target="_blank" rel="noreferrer">github&nbsp;↗</a>
      <button id="theme-toggle" class="icon-btn" aria-label="Toggle theme" title="Toggle theme">
        <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path fill="currentColor" d="M12 18a6 6 0 1 1 0-12 6 6 0 0 1 0 12zm0-2a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM11 1h2v3h-2zM11 20h2v3h-2zM3.5 4.9l1.4-1.4 2.1 2.1-1.4 1.4zM16.9 18.4l1.4-1.4 2.1 2.1-1.4 1.4zM20 11h3v2h-3zM1 11h3v2H1zM18.4 4.9l2.1-2.1 1.4 1.4-2.1 2.1zM4.9 18.4l2.1-2.1 1.4 1.4-2.1 2.1z"/></svg>
      </button>
    </div>
  </div>
</header>`;
}

export function shell(o: ShellOpts): string {
  const isFull = !o.sidebar;
  const desc = o.description || SITE.tagline;
  const layout = isFull
    ? `<main class="full">${o.content}</main>`
    : `<div class="doc-layout">
    <aside class="sidebar" id="sidebar">${o.sidebar}</aside>
    <main class="doc-main">
      <article class="doc-content">${o.content}</article>
      ${o.pager || ""}
      <div class="exit-line"><span class="prompt">llmff@docs</span>:<span class="cwd">~</span>$ <span class="exit0">exit 0</span><span class="cursor"></span></div>
    </main>
    <aside class="toc-rail">${o.toc || ""}</aside>
  </div>`;

  return `<!doctype html>
<html lang="en" data-theme="dark">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>${escapeHtml(o.title)} · ${SITE.name}</title>
<meta name="description" content="${escapeAttr(desc)}"/>
<meta property="og:title" content="${escapeAttr(o.title)} · ${SITE.name}"/>
<meta property="og:description" content="${escapeAttr(desc)}"/>
<link rel="icon" href="${url("/assets/favicon.svg")}"/>
<link rel="preconnect" href="https://fonts.googleapis.com"/>
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin/>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&family=Inter:wght@400;500;600;700&display=swap"/>
<link rel="stylesheet" href="${url("/assets/style.css")}"/>
<link rel="stylesheet" href="${url("/assets/hljs.css")}"/>
</head>
<body class="${o.bodyClass || ""}">
${headerHtml(o.section, o.crumb)}
<div class="page">${layout}</div>
<footer class="site-footer">
  <div class="ft-inner">
    <span class="ft-prompt"><span class="prompt">$</span> llmff --version <span class="ft-dim"># ${SITE.tagline}</span></span>
    <span class="ft-links">
      <a href="${SITE.repo}" target="_blank" rel="noreferrer">github</a>
      <a href="${url("/spec.html")}">spec</a>
      <a href="${url("/research.html")}">research</a>
      <a href="${url("/releases.html")}">releases</a>
    </span>
  </div>
</footer>
<div id="search-modal" class="search-modal" hidden>
  <div class="search-box">
    <div class="sb-head"><span class="prompt">llmff</span> <span class="sb-cmd">search</span> <span class="sb-flag">--query</span></div>
    <input id="search-input" type="search" placeholder="type to filter docs, blog, research…" autocomplete="off" spellcheck="false"/>
    <ul id="search-results"></ul>
    <div class="search-foot"><kbd>↑</kbd><kbd>↓</kbd> navigate · <kbd>↵</kbd> open · <kbd>esc</kbd> close</div>
  </div>
</div>
<script>window.__BASE__=${JSON.stringify(SITE.base)};</script>
<script src="${url("/assets/app.js")}" defer></script>
<script src="${url("/assets/mermaid-init.js")}" type="module"></script>
</body>
</html>`;
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c] ?? c));
}
function escapeAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}
