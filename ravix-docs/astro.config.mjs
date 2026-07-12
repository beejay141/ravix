import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
  integrations: [
    starlight({
      title: "Ravix",
      logo: {
        dark: "./public/logo-dark.svg",
        light: "./public/logo-light.svg",
      },
      editLink: {
        baseUrl: "https://github.com/ravix/ravix/edit/main/docs/",
      },
      head: [
        {
          tag: "meta",
          attrs: {
            name: "description",
            content:
              "Modern Rust API framework with clean architecture and dependency injection",
          },
        },
        {
          tag: "link",
          attrs: {
            rel: "icon",
            type: "image/svg+xml",
            href: "/favicon.svg",
          },
        },
        {
          tag: "meta",
          attrs: {
            property: "og:image",
            content: "https://ravix.dev/og-image.svg",
          },
        },
      ],
      customCss: ["./src/styles/starlight.css"],
      sidebar: [
        {
          label: "Getting Started",
          items: [
            { label: "Introduction", slug: "index" },
            { label: "Installation", slug: "getting-started/installation" },
            { label: "Quick Start", slug: "getting-started/quick-start" },
            {
              label: "Project Structure",
              slug: "getting-started/project-structure",
            },
          ],
        },
        {
          label: "Guides",
          items: [
            { label: "Controllers", slug: "guides/controllers" },
            {
              label: "Dependency Injection",
              slug: "guides/dependency-injection",
            },
            { label: "Middleware", slug: "guides/middleware" },
            { label: "Error Handling", slug: "guides/error-handling" },
            { label: "Testing", slug: "guides/testing" },
            { label: "Deployment", slug: "guides/deployment" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "API Reference", slug: "reference/api" },
            { label: "Response Helpers", slug: "reference/response-helpers" },
            { label: "CORS Configuration", slug: "reference/cors" },
            { label: "APM", slug: "reference/apm" },
            { label: "Logger", slug: "reference/logger" },
          ],
        },
        {
          label: "Examples",
          items: [
            { label: "Blog API Walkthrough", slug: "examples/blog-api" },
            { label: "Microservices Patterns", slug: "examples/microservices" },
          ],
        },
        {
          label: "Integrations",
          collapsed: true,
          items: [
            {
              label: "MongoDB",
              slug: "integrations/mongodb",
              badge: "Coming Soon",
            },
            {
              label: "JWT Authentication",
              slug: "integrations/jwt-auth",
              badge: "Coming Soon",
            },
            {
              label: "Docker",
              slug: "integrations/docker",
              badge: "Coming Soon",
            },
          ],
        },
      ],
      social: {
        github: "https://github.com/ravix/ravix",
        discord: "https://discord.gg/ravix",
      },
    }),
  ],
  site: "https://ravix.dev",
  base: "/",
});
