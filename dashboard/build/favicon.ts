/** The favicon's colour: a browser tab has no theme, and this blue reads on light and dark tab bars alike. */
export const FAVICON_COLOR = '#2f81f7';

/** The logo as a favicon: its comments dropped, currentColor made a fixed colour, as a data: URL. */
export function faviconHref(svg: string, color = FAVICON_COLOR): string {
  const icon = svg
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/currentColor/g, color)
    .replace(/\s+/g, ' ')
    .trim();
  return 'data:image/svg+xml,' + encodeURIComponent(icon);
}

/** The page's favicon, inline (the dashboard is a single file; build/finish-page.ts), made from the logo's source */
export function setFavicon(html: string, href: string): string {
  if (!html.includes('</head>')) throw new Error('no </head> in the page');
  return html.replace('</head>', `  <link rel="icon" type="image/svg+xml" href="${href}" />\n  </head>`);
}
