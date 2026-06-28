/* llmff docs — client behaviors: theme, search, copy, scrollspy, mobile nav. */
(() => {
  const BASE = (window.__BASE__ || "/").replace(/\/$/, "");
  const u = (p) => BASE + (p.startsWith("/") ? p : "/" + p);

  /* ---- theme ---- */
  const root = document.documentElement;
  const saved = localStorage.getItem("llmff-theme");
  if (saved) root.setAttribute("data-theme", saved);
  const tt = document.getElementById("theme-toggle");
  tt && tt.addEventListener("click", () => {
    const next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
    root.setAttribute("data-theme", next);
    localStorage.setItem("llmff-theme", next);
  });

  /* ---- collapsible nav groups ---- */
  document.querySelectorAll(".nav-group > .nav-label").forEach((btn) => {
    btn.addEventListener("click", () => btn.parentElement.classList.toggle("collapsed"));
  });

  /* ---- mobile sidebar ---- */
  const mt = document.getElementById("menu-toggle");
  mt && mt.addEventListener("click", () => document.body.classList.toggle("nav-open"));

  /* ---- copy buttons ---- */
  document.querySelectorAll(".doc-content pre").forEach((pre) => {
    if (pre.classList.contains("mermaid")) return;
    const btn = document.createElement("button");
    btn.className = "copy-btn"; btn.type = "button"; btn.textContent = "Copy";
    btn.addEventListener("click", async () => {
      const code = pre.querySelector("code");
      try {
        await navigator.clipboard.writeText(code ? code.innerText : pre.innerText);
        btn.textContent = "Copied"; btn.classList.add("copied");
        setTimeout(() => { btn.textContent = "Copy"; btn.classList.remove("copied"); }, 1400);
      } catch (_) {}
    });
    pre.appendChild(btn);
  });

  /* ---- TOC scrollspy ---- */
  const tocLinks = [...document.querySelectorAll(".toc-list a")];
  if (tocLinks.length) {
    const map = new Map();
    tocLinks.forEach((a) => {
      const id = decodeURIComponent(a.getAttribute("href").split("#")[1] || "");
      const el = id && document.getElementById(id);
      if (el) map.set(el, a);
    });
    const obs = new IntersectionObserver((entries) => {
      entries.forEach((e) => {
        if (e.isIntersecting) {
          tocLinks.forEach((l) => l.classList.remove("active"));
          const a = map.get(e.target); a && a.classList.add("active");
        }
      });
    }, { rootMargin: "-72px 0px -70% 0px", threshold: 0 });
    map.forEach((_, el) => obs.observe(el));
  }

  /* ---- search ---- */
  let index = null, loading = null;
  const modal = document.getElementById("search-modal");
  const input = document.getElementById("search-input");
  const results = document.getElementById("search-results");
  const openBtn = document.getElementById("search-open");

  const loadIndex = () => {
    if (index) return Promise.resolve(index);
    if (loading) return loading;
    loading = fetch(u("/assets/search-index.json")).then((r) => r.json()).then((d) => (index = d));
    return loading;
  };
  const openSearch = () => {
    if (!modal) return;
    modal.hidden = false; loadIndex(); setTimeout(() => input.focus(), 0);
  };
  const closeSearch = () => { if (modal) { modal.hidden = true; input.value = ""; results.innerHTML = ""; } };

  openBtn && openBtn.addEventListener("click", openSearch);
  modal && modal.addEventListener("click", (e) => { if (e.target === modal) closeSearch(); });

  document.addEventListener("keydown", (e) => {
    if (e.key === "/" && !/input|textarea|select/i.test(document.activeElement.tagName) && (!modal || modal.hidden)) {
      e.preventDefault(); openSearch();
    } else if ((e.key === "k" || e.key === "K") && (e.metaKey || e.ctrlKey)) {
      e.preventDefault(); openSearch();
    } else if (e.key === "Escape" && modal && !modal.hidden) {
      closeSearch();
    }
  });

  let active = -1;
  const score = (q, p) => {
    q = q.toLowerCase();
    const t = p.title.toLowerCase(), b = (p.text || "").toLowerCase();
    let s = 0;
    if (t === q) s += 100;
    if (t.includes(q)) s += 40;
    if (t.startsWith(q)) s += 20;
    const idx = b.indexOf(q);
    if (idx >= 0) s += 12;
    q.split(/\s+/).filter(Boolean).forEach((w) => { if (t.includes(w)) s += 6; if (b.includes(w)) s += 1; });
    return { s, idx };
  };
  const snippet = (text, q) => {
    if (!text) return "";
    const i = text.toLowerCase().indexOf(q.toLowerCase());
    if (i < 0) return text.slice(0, 140);
    const start = Math.max(0, i - 50);
    return (start > 0 ? "…" : "") + text.slice(start, start + 150);
  };
  const render = (q) => {
    if (!index || !q.trim()) { results.innerHTML = ""; active = -1; return; }
    const ranked = index
      .map((p) => ({ p, ...score(q, p) }))
      .filter((r) => r.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 12);
    results.innerHTML = ranked
      .map(
        (r, i) => `<li><a class="${i === 0 ? "active" : ""}" href="${u(r.p.url)}">
          <div class="r-sec">${r.p.section}</div>
          <div class="r-title">${r.p.title}</div>
          <div class="r-snip">${escapeHtml(snippet(r.p.text, q))}</div></a></li>`,
      )
      .join("");
    active = ranked.length ? 0 : -1;
  };
  input && input.addEventListener("input", () => loadIndex().then(() => render(input.value)));
  input && input.addEventListener("keydown", (e) => {
    const items = [...results.querySelectorAll("a")];
    if (!items.length) return;
    if (e.key === "ArrowDown") { e.preventDefault(); active = (active + 1) % items.length; }
    else if (e.key === "ArrowUp") { e.preventDefault(); active = (active - 1 + items.length) % items.length; }
    else if (e.key === "Enter") { e.preventDefault(); items[Math.max(0, active)].click(); return; }
    else return;
    items.forEach((a, i) => a.classList.toggle("active", i === active));
    items[active].scrollIntoView({ block: "nearest" });
  });
  function escapeHtml(s) { return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c])); }
})();
