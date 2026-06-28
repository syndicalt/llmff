// Render mermaid diagrams if any are present. Loaded from CDN lazily.
const nodes = document.querySelectorAll("pre.mermaid, .mermaid");
if (nodes.length) {
  const theme = document.documentElement.getAttribute("data-theme") === "light" ? "neutral" : "dark";
  try {
    const mermaid = (await import("https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs")).default;
    mermaid.initialize({ startOnLoad: false, theme, securityLevel: "loose", fontFamily: "inherit" });
    await mermaid.run({ nodes });
  } catch (_) {
    // Offline: leave the fenced source visible as a code block.
  }
}
