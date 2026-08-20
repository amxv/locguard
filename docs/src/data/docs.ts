export const siteConfig = {
  name: "locguard",
  strapline: "Keep source files small",
  description:
    "A fast, zero-config CLI that keeps source files below a configurable physical line limit.",
  repoUrl: "https://github.com/amxv/locguard",
  accentColor: "#b7410e",
  accentColorDark: "#f28c52",
  footerSections: [
    {
      title: "locguard",
      text: "A tiny source-file size invariant for humans, agents, local checks, and CI."
    },
    {
      title: "Defaults",
      text: "1,000-line limit, warnings at 90%, Git ignores respected, and generated/vendor/build trees skipped."
    },
    {
      title: "Repository",
      linkPrefix: "Source: ",
      linkHref: "https://github.com/amxv/locguard",
      linkLabel: "github.com/amxv/locguard"
    }
  ]
} as const;

export const docCategories = ["Start", "Guide", "Reference"] as const;

export const primaryNav = [
  { href: "/docs", label: "Docs" },
  { href: siteConfig.repoUrl, label: "GitHub", external: true }
];
