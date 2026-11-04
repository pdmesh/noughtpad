// One place for facts that change. Edit these, rebuild, redeploy.

export const SITE_URL = "https://noughtpad.com";
export const SITE_NAME = "Noughtpad";
export const TAGLINE = "A blank page that quietly remembers.";

/** Current release. Bump with each published release. */
export const VERSION = "0.1.5";

/**
 * Where installers live. When releases move to the main repo, change this one line
 * to "https://github.com/pdmesh/noughtpad/releases/latest".
 */
export const RELEASES_URL = "https://github.com/pdmesh/noughtpad-releases/releases/latest";

/**
 * Link to the source code. Leave null while the repo is private; set it to
 * "https://github.com/pdmesh/noughtpad" once it is public; the GitHub icon in the top bar, the footer link and the licence links all appear.
 */
export const SOURCE_URL: string | null = null;

/** No email address on the site (spam). People write through /contact/, which emails us. */

/** The date the comparison pages were checked against each app's own site. */
export const COMPARISONS_CHECKED = "4 November 2026";

/**
 * Legal pages (privacy, terms). Fill these in before launch:
 * - LEGAL_NAME: the person or company that offers Noughtpad, as it should appear in the terms.
 * - GOVERNING_LAW: e.g. "the laws of England and Wales" or "the laws of India". Left null,
 *   the terms leave out the governing-law clause.
 */
export const LEGAL_NAME: string | null = null;
export const GOVERNING_LAW: string | null = null;
export const LEGAL_UPDATED = "4 November 2026";
