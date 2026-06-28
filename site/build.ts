/* llmff docs static site generator.
   Collects markdown across the repo, renders it with a terminal-styled shell,
   rewrites internal links/assets, and emits a self-contained static site. */
import { promises as fs } from "node:fs";
import { existsSync, statSync } from "node:fs";
import path from "node:path";
import MarkdownIt from "markdown-it";
import anchor from "markdown-it-anchor";
import hljs from "highlight.js";
import matter from "gray-matter";
import { shell, escapeHtml, url, SITE, type TocEntry } from "./theme.ts";
import { SECTIONS, type NavGroup, type Section } from "./nav.ts";

const HERE = import.meta.dir;
const ROOT = path.resolve(HERE, "..");
const OUT = path.join(HERE, "dist");
const ASSET_SRC = path.join(HERE, "assets");
const CONTENT = path.join(HERE, "content");

type Layout = "doc" | "full";
type SectionId = "docs" | "blog" | "research" | "releases" | "home";

interface Page {
  srcRel: string;            // repo-relative key for link resolution, e.g. "docs/cookbook.md"
  srcAbs: string;
  outPath: string;           // dist-relative, e.g. "docs/cookbook.html"
  href: string;              // base-less root-relative url, e.g. "/docs/cookbook.html"
  section: SectionId;
  layout: Layout;
  title: string;
  fm: Record<string, unknown>;
  body: string;              // markdown (no frontmatter)
}

const assetsToCopy = new Set<string>();
let currentToc: TocEntry[] = [];

/* ---------------- frontmatter helpers ---------------- */
function fmStr(fm: Record<string, unknown>, key: string): string | undefined {
  const v = fm[key];
  return typeof v === "string" ? v : undefined;
}
function fmArr(fm: Record<string, unknown>, key: string): string[] {
  const v = fm[key];
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
}

function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/<[^>]*>/g, "")
    .replace(/[^\w\s-]+/g, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

/* ---------------- markdown engine ---------------- */
const md: MarkdownIt = new MarkdownIt({
  html: true,
  linkify: true,
  typographer: false,
  highlight: (code: string, lang: string): string => {
    if (lang === "mermaid") return `<pre class="mermaid">${escapeHtml(code)}</pre>`;
    if (lang && hljs.getLanguage(lang)) {
      try {
        const out = hljs.highlight(code, { language: lang, ignoreIllegals: true }).value;
        return `<pre><code class="hljs language-${lang}">${out}</code></pre>`;
      } catch {
        /* fall through */
      }
    }
    return `<pre><code class="hljs">${escapeHtml(code)}</code></pre>`;
  },
});

md.use(anchor, {
  level: [2, 3, 4],
  slugify,
  permalink: anchor.permalink.linkInsideHeader({
    symbol: "#",
    placement: "after",
    class: "anchor-link",
    ariaHidden: true,
  }),
  callback: (token: { tag: string }, info: { slug: string; title: string }): void => {
    currentToc.push({ level: Number(token.tag.slice(1)) || 2, text: info.title, slug: info.slug });
  },
});

function hasSrcRel(e: unknown): e is { srcRel: string } {
  return typeof e === "object" && e !== null && "srcRel" in e && typeof (e as Record<string, unknown>).srcRel === "string";
}

const EXTERNAL = /^(https?:|mailto:|tel:|data:|\/\/|#)/i;

function resolveHref(srcRel: string, raw: string, isImage: boolean): string {
  if (!raw || EXTERNAL.test(raw)) return raw;
  const hashIdx = raw.indexOf("#");
  const frag = hashIdx >= 0 ? raw.slice(hashIdx) : "";
  const pathPart = hashIdx >= 0 ? raw.slice(0, hashIdx) : raw;
  if (!pathPart) return raw;
  const targetRel = path.posix.normalize(path.posix.join(path.posix.dirname(srcRel), pathPart));
  const page = pageByRel.get(targetRel) || pageByRel.get(targetRel + "/README.md");
  if (page) return url(page.href) + frag;
  // Treat as a static asset only if it exists on disk and is a file.
  const abs = path.join(ROOT, targetRel);
  if (existsSync(abs) && statSync(abs).isFile()) {
    assetsToCopy.add(targetRel);
    return url("/" + targetRel) + frag;
  }
  void isImage;
  return raw;
}

// Rewrite links and images to point at generated pages / copied assets.
md.renderer.rules.link_open = (tokens, idx, options, env: unknown, self) => {
  const token = tokens[idx];
  const href = token.attrGet("href");
  if (href !== null) {
    const srcRel = hasSrcRel(env) ? env.srcRel : "";
    token.attrSet("href", resolveHref(srcRel, href, false));
    if (/^https?:/i.test(href)) {
      token.attrSet("target", "_blank");
      token.attrSet("rel", "noreferrer");
    }
  }
  return self.renderToken(tokens, idx, options);
};
md.renderer.rules.image = (tokens, idx, options, env: unknown, self) => {
  const token = tokens[idx];
  const src = token.attrGet("src");
  if (src !== null) {
    const srcRel = hasSrcRel(env) ? env.srcRel : "";
    token.attrSet("src", resolveHref(srcRel, src, true));
  }
  return self.renderToken(tokens, idx, options);
};

function renderMarkdown(srcRel: string, body: string): { html: string; toc: TocEntry[] } {
  currentToc = [];
  const html = md.render(body, { srcRel });
  const withCallouts = html.replace(
    /<p>\s*\[IMAGE PLACEHOLDER:\s*([\s\S]*?)\]\s*<\/p>/g,
    (_m, cap: string) => `<div class="callout placeholder"><span class="ico">▣ figure</span><span>${cap.trim()}</span></div>`,
  );
  return { html: withCallouts, toc: currentToc.slice() };
}

/* ---------------- output path mapping ---------------- */
function outFor(srcRel: string): { outPath: string; href: string } {
  const mk = (outPath: string) => ({ outPath, href: "/" + outPath });
  const blog = srcRel.match(/^blog\/([^/]+)\/article\.md$/);
  if (blog) return mk(`blog/${blog[1]}.html`);
  if (srcRel.startsWith("content/")) {
    const name = srcRel.slice("content/".length).replace(/\.md$/, ".html");
    return mk(name === "home.html" ? "index.html" : name);
  }
  if (!srcRel.includes("/")) return mk(srcRel.replace(/\.md$/, ".html").toLowerCase());
  return mk(srcRel.replace(/\.md$/, ".html"));
}

function sectionFor(srcRel: string): SectionId {
  if (srcRel === "content/home.md") return "home";
  if (srcRel === "content/research.md") return "research";
  if (srcRel.startsWith("docs/release-notes/")) return "releases";
  if (srcRel.startsWith("blog/")) return "blog";
  return "docs";
}

function deriveTitle(fm: Record<string, unknown>, body: string, srcRel: string): string {
  const t = fmStr(fm, "title");
  if (t) return t;
  const h1 = body.match(/^#\s+(.+?)\s*$/m);
  if (h1) return h1[1].replace(/`/g, "");
  return path.basename(srcRel).replace(/\.md$/, "");
}

/* ---------------- collection ---------------- */
async function walk(dir: string): Promise<string[]> {
  const acc: string[] = [];
  if (!existsSync(dir)) return acc;
  for (const e of await fs.readdir(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) acc.push(...(await walk(p)));
    else acc.push(p);
  }
  return acc;
}

const pages: Page[] = [];
const pageByRel = new Map<string, Page>();

async function addSource(srcRel: string, layoutHint?: Layout): Promise<void> {
  const srcAbs = srcRel.startsWith("content/") ? path.join(HERE, srcRel) : path.join(ROOT, srcRel);
  if (!existsSync(srcAbs)) return;
  if (pageByRel.has(srcRel)) return;
  const rawText = await fs.readFile(srcAbs, "utf8");
  const parsed = matter(rawText);
  const fm: Record<string, unknown> = parsed.data;
  const section = sectionFor(srcRel);
  const layout: Layout = layoutHint ?? (section === "home" ? "full" : "doc");
  const { outPath, href } = outFor(srcRel);
  const page: Page = {
    srcRel,
    srcAbs,
    outPath,
    href,
    section,
    layout,
    title: deriveTitle(fm, parsed.content, srcRel),
    fm,
    body: parsed.content,
  };
  pages.push(page);
  pageByRel.set(srcRel, page);
}

async function collect(): Promise<void> {
  // authored content
  await addSource("content/home.md", "full");
  await addSource("content/research.md", "full");
  // root docs
  for (const f of ["README.md", "SPEC.md", "CONTRIBUTING.md"]) await addSource(f);
  // all docs
  for (const abs of await walk(path.join(ROOT, "docs"))) {
    if (abs.endsWith(".md")) await addSource(path.relative(ROOT, abs));
  }
  // examples docs
  for (const abs of await walk(path.join(ROOT, "examples"))) {
    if (abs.endsWith(".md")) await addSource(path.relative(ROOT, abs));
  }
  // blog
  for (const abs of await walk(path.join(ROOT, "blog"))) {
    const rel = path.relative(ROOT, abs);
    if (rel.endsWith("article.md")) await addSource(rel);
  }
}

/* ---------------- nav rendering ---------------- */
const CHEV = `<svg class="chev" viewBox="0 0 16 16" width="11" height="11" aria-hidden="true"><path fill="currentColor" d="M6 4l4 4-4 4z"/></svg>`;

function navItemsHtml(items: { href: string; title: string }[], activeHref: string): string {
  return items
    .map(
      (it) =>
        `<li><a class="${it.href === activeHref ? "active" : ""}" href="${url(it.href)}">${escapeHtml(it.title)}</a></li>`,
    )
    .join("");
}

function docsGroups(): NavGroup[] {
  const docsSection = SECTIONS.find((s) => s.id === "docs") as Section;
  const groups = docsSection.groups.map((g) => ({ ...g, items: g.items.filter((i) => pageByRel.has(i.src)) }));
  // auto-collect ungrouped docs so nothing is dropped
  const referenced = new Set(docsSection.groups.flatMap((g) => g.items.map((i) => i.src)));
  const extra = pages
    .filter((p) => p.section === "docs" && !referenced.has(p.srcRel))
    .filter((p) => p.srcRel !== "content/home.md")
    .sort((a, b) => a.srcRel.localeCompare(b.srcRel));
  const internalRe = /^docs\/(superpowers|release-evidence)\//;
  const exampleRe = /^examples\//;
  const push = (label: string, list: Page[], collapsed: boolean) => {
    if (list.length) {
      groups.push({ label, collapsed, items: list.map((p) => ({ src: p.srcRel, title: p.title })) });
    }
  };
  push("More Examples", extra.filter((p) => exampleRe.test(p.srcRel)), true);
  push("More Documentation", extra.filter((p) => !exampleRe.test(p.srcRel) && !internalRe.test(p.srcRel)), true);
  push("Design Notes (internal)", extra.filter((p) => internalRe.test(p.srcRel)), true);
  return groups;
}

function docsSidebar(activeHref: string): string {
  const groups = docsGroups();
  const blocks = groups
    .map((g) => {
      const items = g.items
        .map((i) => pageByRel.get(i.src))
        .filter((p): p is Page => Boolean(p))
        .map((p, idx) => ({ href: p.href, title: g.items[idx]?.title ?? p.title }));
      const containsActive = items.some((i) => i.href === activeHref);
      const collapsed = g.collapsed && !containsActive;
      return `<div class="nav-group${collapsed ? " collapsed" : ""}">
        <button class="nav-label">${CHEV}${escapeHtml(g.label)}</button>
        <ul class="nav-items">${navItemsHtml(items, activeHref)}</ul>
      </div>`;
    })
    .join("");
  return `<div class="sb-prompt"><span class="prompt">$</span> <span class="cmd">llmff</span> docs <span style="color:var(--text-faint)">--tree</span></div>${blocks}`;
}

function listSidebar(label: string, items: { href: string; title: string }[], activeHref: string, cmd: string): string {
  return `<div class="sb-prompt"><span class="prompt">$</span> <span class="cmd">llmff</span> ${cmd}</div>
    <div class="nav-group"><button class="nav-label">${CHEV}${escapeHtml(label)}</button>
    <ul class="nav-items">${navItemsHtml(items, activeHref)}</ul></div>`;
}

function tocHtml(toc: TocEntry[]): string {
  if (toc.length < 2) return "";
  const items = toc
    .map(
      (t) =>
        `<li><a class="lvl-${t.level}" href="#${t.slug}">${escapeHtml(t.text)}</a></li>`,
    )
    .join("");
  return `<div class="toc-title">on this page</div><ul class="toc-list">${items}</ul>`;
}

function pagerHtml(ordered: Page[], current: Page): string {
  const i = ordered.findIndex((p) => p.srcRel === current.srcRel);
  if (i < 0) return "";
  const prev = ordered[i - 1];
  const next = ordered[i + 1];
  const a = prev
    ? `<a class="prev" href="${url(prev.href)}"><div class="dir">← prev</div><div class="pg-title">${escapeHtml(prev.title)}</div></a>`
    : `<span style="flex:1"></span>`;
  const b = next
    ? `<a class="next" href="${url(next.href)}"><div class="dir">next →</div><div class="pg-title">${escapeHtml(next.title)}</div></a>`
    : `<span style="flex:1"></span>`;
  return `<nav class="pager">${a}${b}</nav>`;
}

/* ---------------- ordered lists per section ---------------- */
function orderedDocs(): Page[] {
  const out: Page[] = [];
  for (const g of docsGroups()) {
    for (const i of g.items) {
      const p = pageByRel.get(i.src);
      if (p) out.push(p);
    }
  }
  return out;
}
function blogPages(): Page[] {
  return pages.filter((p) => p.section === "blog").sort((a, b) => a.srcRel.localeCompare(b.srcRel));
}
function releasePages(): Page[] {
  // newest first by version-ish filename; x-articles after their note
  return pages
    .filter((p) => p.section === "releases")
    .sort((a, b) => b.srcRel.localeCompare(a.srcRel, undefined, { numeric: true }));
}

/* ---------------- search index ---------------- */
interface SearchDoc { title: string; section: string; url: string; text: string; }
const searchDocs: SearchDoc[] = [];

function stripHtml(html: string): string {
  return html
    .replace(/<pre[\s\S]*?<\/pre>/g, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&amp;/g, "&").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'")
    .replace(/\s+/g, " ")
    .trim();
}

// First real prose paragraph of a markdown body, for list summaries.
function firstPara(body: string): string {
  const lines = body.split(/\r?\n/);
  const buf: string[] = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) { if (buf.length) break; else continue; }
    if (line.startsWith("#") || line.startsWith("[IMAGE PLACEHOLDER") || line.startsWith("<") || line.startsWith("```") || line.startsWith("---")) {
      if (buf.length) break; else continue;
    }
    buf.push(line);
  }
  return buf
    .join(" ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

const SECTION_LABEL: Record<SectionId, string> = {
  docs: "docs", blog: "blog", research: "research", releases: "releases", home: "home",
};

/* ---------------- emit ---------------- */
async function writeOut(rel: string, contents: string): Promise<void> {
  const dest = path.join(OUT, rel);
  await fs.mkdir(path.dirname(dest), { recursive: true });
  await fs.writeFile(dest, contents);
}

function crumbFor(p: Page): string {
  if (p.section === "home") return "";
  if (p.section === "blog") return "blog/" + path.basename(p.outPath, ".html");
  if (p.section === "releases") return "releases/" + path.basename(p.outPath, ".html");
  return p.outPath.replace(/\.html$/, "");
}

// Prefix the deploy base onto root-relative href/src (no-op when base is "/").
// Used only for authored bodies, which hand-write base-less absolute links.
function withBase(html: string): string {
  const b = SITE.base.replace(/\/$/, "");
  if (!b) return html;
  return html.replace(/(href|src)="\/(?!\/)/g, `$1="${b}/`);
}

async function renderPage(p: Page): Promise<void> {
  const { html, toc } = renderMarkdown(p.srcRel, p.body);
  const desc = fmStr(p.fm, "summary") || fmStr(p.fm, "description");
  searchDocs.push({
    title: p.title,
    section: SECTION_LABEL[p.section],
    url: p.href,
    text: stripHtml(html).slice(0, 1400),
  });

  if (p.layout === "full") {
    // Authored full-width pages contain hand-written root-relative links;
    // prefix the deploy base (no-op when base is "/") since they bypass the
    // markdown link rewriter.
    await writeOut(
      p.outPath,
      shell({ title: p.title, description: desc, content: withBase(html), section: p.section === "home" ? undefined : p.section, crumb: crumbFor(p) }),
    );
    return;
  }

  let sidebar = "";
  let ordered: Page[] = [];
  if (p.section === "docs") {
    sidebar = docsSidebar(p.href);
    ordered = orderedDocs();
  } else if (p.section === "blog") {
    sidebar = listSidebar("blog/", blogPages().map((b) => ({ href: b.href, title: b.title })), p.href, "blog --list");
    ordered = blogPages();
  } else if (p.section === "releases") {
    sidebar = listSidebar("release-notes/", releasePages().map((r) => ({ href: r.href, title: r.title })), p.href, "releases");
    ordered = releasePages();
  }

  const meta = blogMeta(p);
  await writeOut(
    p.outPath,
    shell({
      title: p.title,
      description: desc,
      content: meta + html,
      sidebar,
      toc: tocHtml(toc),
      section: p.section === "home" ? undefined : p.section,
      pager: pagerHtml(ordered, p),
      crumb: crumbFor(p),
    }),
  );
}

function blogMeta(p: Page): string {
  if (p.section !== "blog") return "";
  const date = fmStr(p.fm, "date");
  const source = fmStr(p.fm, "source");
  const tags = fmArr(p.fm, "tags");
  const bits: string[] = [];
  if (date) bits.push(date);
  if (source) bits.push("via " + source);
  if (tags.length) bits.push(tags.join(", "));
  if (!bits.length) return "";
  return `<div class="doc-meta">${bits.map(escapeHtml).join('<span class="sep">·</span>')}</div>`;
}

/* index pages (generated) */
async function renderBlogIndex(): Promise<void> {
  const posts = blogPages();
  const cards = posts
    .map((p, i) => {
      const n = String(i + 1).padStart(2, "0");
      const date = fmStr(p.fm, "date") || "";
      const summary = fmStr(p.fm, "summary") || firstPara(p.body).slice(0, 180) + "…";
      const tags = fmArr(p.fm, "tags").map((t) => `<span class="tag">${escapeHtml(t)}</span>`).join("");
      return `<article class="post-card">
        <div class="pc-meta"><span class="n">post #${n}</span>${date ? " · " + escapeHtml(date) : ""}</div>
        <h3><a href="${url(p.href)}">${escapeHtml(p.title)}</a></h3>
        <p>${escapeHtml(summary)}</p>
        ${tags ? `<div class="tags">${tags}</div>` : ""}
      </article>`;
    })
    .join("");
  const content = `<section class="section">
    <div class="page-head"><div class="kicker">$ llmff blog --list</div>
    <h1>The llmff Blog</h1>
    <p>Field notes on building a bounded execution layer for LLM pipelines — the design decisions, the boundaries, and why structured graphs beat another agent loop.</p></div>
    <div class="post-list">${cards}</div>
  </section>`;
  await writeOut("blog/index.html", shell({ title: "Blog", description: "Field notes on the llmff execution layer.", content, section: "blog", crumb: "blog" }));
}

async function renderReleasesIndex(): Promise<void> {
  const notes = releasePages().filter((p) => !p.srcRel.includes("-x-article"));
  const items = notes
    .map((p) => {
      const ver = p.title.replace(/^llmff\s*/i, "");
      const summary = firstPara(p.body).slice(0, 200) + "…";
      return `<a class="rel-item" href="${url(p.href)}">
        <div class="rel-tag">${escapeHtml(ver)}</div>
        <div><h3>${escapeHtml(p.title)}</h3><p>${escapeHtml(summary)}</p></div>
      </a>`;
    })
    .join("");
  const content = `<section class="section">
    <div class="page-head"><div class="kicker">$ llmff releases</div>
    <h1>Releases</h1>
    <p>Versioned release notes for llmff. Each tag ships inspect-stable contracts: CLI flags, schemas, events, traces, and exit codes evolve additively.</p></div>
    <div class="rel-list">${items}</div>
  </section>`;
  await writeOut("releases.html", shell({ title: "Releases", description: "llmff release notes.", content, section: "releases", crumb: "releases" }));
}

async function copyAssets(): Promise<void> {
  await fs.mkdir(path.join(OUT, "assets"), { recursive: true });
  for (const f of await fs.readdir(ASSET_SRC)) {
    await fs.copyFile(path.join(ASSET_SRC, f), path.join(OUT, "assets", f));
  }
  for (const rel of assetsToCopy) {
    const from = path.join(ROOT, rel);
    if (!existsSync(from) || !statSync(from).isFile()) continue;
    const to = path.join(OUT, rel);
    await fs.mkdir(path.dirname(to), { recursive: true });
    await fs.copyFile(from, to);
  }
  await writeOut("assets/search-index.json", JSON.stringify(searchDocs));
}

async function main(): Promise<void> {
  await fs.rm(OUT, { recursive: true, force: true });
  await fs.mkdir(OUT, { recursive: true });
  await collect();
  for (const p of pages) await renderPage(p);
  await renderBlogIndex();
  await renderReleasesIndex();
  await copyAssets();
  // 404
  await writeOut(
    "404.html",
    shell({ title: "404", content: `<section class="section center" style="text-align:center"><h1 style="font-family:var(--mono)"><span class="prompt">$</span> llmff: no such file</h1><p class="sub">That page exited with code 1. <a href="${url("/index.html")}">cd ~/llmff</a></p></section>` }),
  );
  console.log(`built ${pages.length} pages + blog/releases indexes → ${path.relative(ROOT, OUT)}`);
  console.log(`search index: ${searchDocs.length} docs · assets copied: ${assetsToCopy.size}`);
}

await main();
