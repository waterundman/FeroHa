export interface WikiLink {
  target: string;
  display: string;
  heading: string | null;
  position: {
    start: number;
    end: number;
  };
}

const WIKI_LINK_REGEX = /\[\[([^\]]+)\]\]/g;

/**
 * Parse wiki links from content
 * Supports: [[note]], [[note|display]], [[note#heading]], [[note#heading|display]]
 * @param content - The content to parse
 * @returns Array of WikiLink objects
 */
export function parseWikiLinks(content: string): WikiLink[] {
  const links: WikiLink[] = [];
  let match: RegExpExecArray | null;

  while ((match = WIKI_LINK_REGEX.exec(content)) !== null) {
    const inner = match[1];
    const start = match.index;
    const end = start + match[0].length;

    let target = inner;
    let display = inner;
    let heading: string | null = null;

    const pipeIndex = inner.indexOf("|");
    const hashIndex = inner.indexOf("#");

    if (pipeIndex !== -1) {
      target = inner.substring(0, pipeIndex).trim();
      display = inner.substring(pipeIndex + 1).trim();
      const hashInTarget = target.indexOf("#");
      if (hashInTarget !== -1) {
        heading = target.substring(hashInTarget + 1).trim();
        target = target.substring(0, hashInTarget).trim();
      }
    } else if (hashIndex !== -1) {
      target = inner.substring(0, hashIndex).trim();
      heading = inner.substring(hashIndex + 1).trim();
      display = target;
    }

    // Skip empty or whitespace-only targets
    if (!target.trim()) continue;

    links.push({ target, display, heading, position: { start, end } });
  }

  return links;
}