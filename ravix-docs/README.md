# Ravix Documentation Website

This is the Astro-based documentation website for the Ravix framework.

## Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Structure

- `src/content/docs/` - Documentation pages in Markdown/MDX
- `src/styles/` - Custom CSS styles
- `public/` - Static assets (logos, images)

## Adding New Pages

Create a new `.md` or `.mdx` file in `src/content/docs/` with frontmatter:

```markdown
---
title: Your Page Title
---

# Your Page Title

Content here...
```

## Deployment

The site is configured for deployment to GitHub Pages or Vercel. Set the `site` URL in `astro.config.mjs` accordingly.
