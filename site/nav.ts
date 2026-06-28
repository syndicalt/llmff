// Site structure + nav grouping. Maps repo-relative source paths to nav
// sections, titles, and ordering. Anything renderable but unlisted is
// auto-appended to the "More" group so nothing is silently dropped.

export interface NavItem {
  /** repo-relative source path, or authored content id like "content/home.md" */
  src: string;
  /** override display title (else derived from frontmatter/H1) */
  title?: string;
}

export interface NavGroup {
  label: string;
  /** collapse by default in the sidebar */
  collapsed?: boolean;
  items: NavItem[];
}

// Top-level site sections shown in the header bar.
export interface Section {
  id: string;
  label: string;
  /** landing url for the header link */
  home: string;
  groups: NavGroup[];
}

export const SECTIONS: Section[] = [
  {
    id: "docs",
    label: "Docs",
    home: "/docs/quickstart.html",
    groups: [
      {
        label: "Get Started",
        items: [
          { src: "docs/quickstart.md", title: "Quickstart" },
          { src: "docs/when-to-use-llmff.md", title: "When to Use llmff" },
          { src: "docs/cookbook.md", title: "Cookbook" },
          { src: "README.md", title: "README" },
          { src: "SPEC.md", title: "Specification" },
        ],
      },
      {
        label: "Concepts & Execution",
        items: [
          { src: "docs/execution.md", title: "Execution Controls" },
          { src: "docs/pipeline-library.md", title: "Pipeline Library" },
          { src: "docs/events.md", title: "Events" },
          { src: "docs/observability.md", title: "Observability" },
          { src: "docs/opentelemetry-bridge.md", title: "OpenTelemetry Bridge" },
          { src: "docs/manifest-reproducibility.md", title: "Manifest Reproducibility" },
        ],
      },
      {
        label: "Agents & Integration",
        items: [
          { src: "docs/agent-workflows.md", title: "Agent Workflows" },
          { src: "docs/agent-harness-contract.md", title: "Agent Harness Contract" },
          { src: "docs/adoption/agent-runner.md", title: "Agent Runner Adoption" },
        ],
      },
      {
        label: "Plugins",
        items: [
          { src: "docs/plugins.md", title: "Plugins" },
          { src: "docs/plugins/registry.md", title: "Plugin Registry" },
          { src: "docs/plugins/promotion-policy.md", title: "Promotion Policy" },
          { src: "docs/plugins/trust.md", title: "Plugin Trust" },
        ],
      },
      {
        label: "Providers",
        collapsed: true,
        items: [
          { src: "docs/providers/support-tiers.md", title: "Support Tiers" },
          { src: "docs/providers/openai.md", title: "OpenAI" },
          { src: "docs/providers/openai-compatible.md", title: "OpenAI-compatible" },
          { src: "docs/providers/azure-openai.md", title: "Azure OpenAI" },
          { src: "docs/providers/anthropic.md", title: "Anthropic" },
          { src: "docs/providers/ollama.md", title: "Ollama" },
          { src: "docs/providers/openrouter.md", title: "OpenRouter" },
          { src: "docs/providers/groq.md", title: "Groq" },
          { src: "docs/providers/together.md", title: "Together" },
          { src: "docs/providers/vllm.md", title: "vLLM" },
          { src: "docs/providers/lm-studio.md", title: "LM Studio" },
          { src: "docs/providers/localai.md", title: "LocalAI" },
          { src: "docs/provider-troubleshooting.md", title: "Troubleshooting" },
          { src: "docs/provider-smoke-readiness.md", title: "Smoke Readiness" },
        ],
      },
      {
        label: "Reference",
        items: [
          { src: "docs/v1-contract.md", title: "v1 Contract" },
          { src: "docs/compatibility/core-contract-v1.md", title: "Core Contract v1" },
          { src: "docs/schemas/README.md", title: "Schemas" },
          { src: "docs/migration/pre-1.0-to-1.0.md", title: "Migration: pre-1.0 → 1.0" },
          { src: "docs/platform-support.md", title: "Platform Support" },
          { src: "docs/github-release-installation.md", title: "Release Installation" },
        ],
      },
      {
        label: "Project & Governance",
        collapsed: true,
        items: [
          { src: "docs/roadmap.md", title: "Roadmap" },
          { src: "docs/governance.md", title: "Governance" },
          { src: "docs/release-readiness.md", title: "Release Readiness" },
          { src: "docs/release-runbook.md", title: "Release Runbook" },
          { src: "docs/ecosystem-readiness.md", title: "Ecosystem Readiness" },
          { src: "docs/distribution-trust.md", title: "Distribution Trust" },
          { src: "docs/package-manager-roadmap.md", title: "Package Manager Roadmap" },
          { src: "docs/apt-repository-design.md", title: "APT Repository Design" },
          { src: "CONTRIBUTING.md", title: "Contributing" },
        ],
      },
      {
        label: "Examples",
        items: [
          { src: "examples/README.md", title: "Example Catalog" },
          { src: "examples/loops/README.md", title: "Loop Examples" },
          { src: "examples/multi-agent/README.md", title: "Multi-agent Examples" },
          { src: "examples/real-world/README.md", title: "Real-world Examples" },
          { src: "examples/agent-harnesses/README.md", title: "Agent Harnesses" },
        ],
      },
    ],
  },
  {
    id: "blog",
    label: "Blog",
    home: "/blog/index.html",
    groups: [],
  },
  {
    id: "research",
    label: "Research",
    home: "/research.html",
    groups: [],
  },
  {
    id: "releases",
    label: "Releases",
    home: "/releases.html",
    groups: [],
  },
];

// Header nav order for the top sections.
export const HEADER = ["docs", "blog", "research", "releases"];
