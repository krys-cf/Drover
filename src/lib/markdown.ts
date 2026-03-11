// Custom markdown renderer — GFM subset, zero dependencies
// Replaces marked — handles headings, paragraphs, code blocks, lists,
// bold/italic, links, images, tables, blockquotes, hr, task lists,
// strikethrough, frontmatter stripping, and MDX preprocessing

import { highlightToHtml } from './highlight';

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function stripFrontmatter(content: string): { body: string; meta: Record<string, string> | null } {
  const match = content.match(/^---\s*\n([\s\S]*?)\n---\s*\n/);
  if (!match) return { body: content, meta: null };
  const meta: Record<string, string> = {};
  for (const line of match[1].split('\n')) {
    const colon = line.indexOf(':');
    if (colon > 0) {
      const key = line.slice(0, colon).trim();
      const val = line.slice(colon + 1).trim();
      meta[key] = val;
    }
  }
  return { body: content.slice(match[0].length), meta };
}

export function preprocessMdx(content: string): string {
  let result = content;
  // Strip import statements
  result = result.replace(/^import\s+.*$/gm, '');
  // ::: directive blocks (callout/admonition style)
  result = result.replace(/:::(\w+)(?:\[([^\]]*)\])?\s*\n([\s\S]*?):::/g,
    (_match, type, label, body) => {
      const heading = label || type;
      return `<div class="md-callout md-callout-${escapeHtml(type)}"><div class="md-callout-title">${escapeHtml(heading)}</div>\n\n${body}\n</div>`;
    }
  );
  // Self-closing JSX tags: <Component prop="val" />
  result = result.replace(/<([A-Z]\w*)\s+[^>]*\/>/g, (_match, tag) => {
    return `\`<${tag} />\``;
  });
  // JSX tags with children: <Component prop="val">children</Component>
  result = result.replace(/<([A-Z]\w*)\s+[^>]*>([\s\S]*?)<\/\1>/g, (_match, tag, children) => {
    const inner = children.trim();
    if (inner) return `\`<${tag}>\` ${inner} \`</${tag}>\``;
    return `\`<${tag} />\``;
  });
  return result;
}

// ── Inline parsing ──

function parseInline(text: string): string {
  let result = '';
  let i = 0;
  const len = text.length;

  while (i < len) {
    // Escaped character
    if (text[i] === '\\' && i + 1 < len) {
      result += escapeHtml(text[i + 1]);
      i += 2;
      continue;
    }

    // Inline code
    if (text[i] === '`') {
      const end = text.indexOf('`', i + 1);
      if (end !== -1) {
        result += `<code>${escapeHtml(text.slice(i + 1, end))}</code>`;
        i = end + 1;
        continue;
      }
    }

    // Images: ![alt](url)
    if (text[i] === '!' && text[i + 1] === '[') {
      const altEnd = text.indexOf(']', i + 2);
      if (altEnd !== -1 && text[altEnd + 1] === '(') {
        const urlEnd = text.indexOf(')', altEnd + 2);
        if (urlEnd !== -1) {
          const alt = escapeHtml(text.slice(i + 2, altEnd));
          const url = escapeHtml(text.slice(altEnd + 2, urlEnd));
          result += `<img src="${url}" alt="${alt}" />`;
          i = urlEnd + 1;
          continue;
        }
      }
    }

    // Links: [text](url)
    if (text[i] === '[') {
      const labelEnd = text.indexOf(']', i + 1);
      if (labelEnd !== -1 && text[labelEnd + 1] === '(') {
        const urlEnd = text.indexOf(')', labelEnd + 2);
        if (urlEnd !== -1) {
          const label = parseInline(text.slice(i + 1, labelEnd));
          const url = escapeHtml(text.slice(labelEnd + 2, urlEnd));
          result += `<a href="${url}">${label}</a>`;
          i = urlEnd + 1;
          continue;
        }
      }
    }

    // Bold + Italic: ***text*** or ___text___
    if ((text[i] === '*' || text[i] === '_') && text[i + 1] === text[i] && text[i + 2] === text[i]) {
      const marker = text[i];
      const end = text.indexOf(marker + marker + marker, i + 3);
      if (end !== -1) {
        result += `<strong><em>${parseInline(text.slice(i + 3, end))}</em></strong>`;
        i = end + 3;
        continue;
      }
    }

    // Bold: **text** or __text__
    if ((text[i] === '*' || text[i] === '_') && text[i + 1] === text[i]) {
      const marker = text[i];
      const end = text.indexOf(marker + marker, i + 2);
      if (end !== -1 && end > i + 2) {
        result += `<strong>${parseInline(text.slice(i + 2, end))}</strong>`;
        i = end + 2;
        continue;
      }
    }

    // Italic: *text* or _text_
    if (text[i] === '*' || text[i] === '_') {
      const marker = text[i];
      const end = text.indexOf(marker, i + 1);
      if (end !== -1 && end > i + 1) {
        result += `<em>${parseInline(text.slice(i + 1, end))}</em>`;
        i = end + 1;
        continue;
      }
    }

    // Strikethrough: ~~text~~
    if (text[i] === '~' && text[i + 1] === '~') {
      const end = text.indexOf('~~', i + 2);
      if (end !== -1) {
        result += `<del>${parseInline(text.slice(i + 2, end))}</del>`;
        i = end + 2;
        continue;
      }
    }

    // Autolinks: <https://...> or <http://...>
    if (text[i] === '<') {
      const end = text.indexOf('>', i + 1);
      if (end !== -1) {
        const inner = text.slice(i + 1, end);
        if (/^https?:\/\//.test(inner)) {
          result += `<a href="${escapeHtml(inner)}">${escapeHtml(inner)}</a>`;
          i = end + 1;
          continue;
        }
      }
    }

    // Bare URLs
    if (text.slice(i).match(/^https?:\/\/\S+/)) {
      const m = text.slice(i).match(/^https?:\/\/[^\s)>\]]+/)!;
      result += `<a href="${escapeHtml(m[0])}">${escapeHtml(m[0])}</a>`;
      i += m[0].length;
      continue;
    }

    result += escapeHtml(text[i]);
    i++;
  }

  return result;
}

// ── Block parsing ──

interface Block {
  type: string;
  content?: string;
  html?: string;
  lang?: string;
  level?: number;
  items?: ListItem[];
  ordered?: boolean;
  rows?: string[][];
  alignments?: string[];
}

interface ListItem {
  content: string;
  checked?: boolean | null;
  children?: Block[];
}

function parseBlocks(text: string): Block[] {
  const lines = text.split('\n');
  const blocks: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Blank line
    if (/^\s*$/.test(line)) {
      i++;
      continue;
    }

    // HTML passthrough (div blocks from MDX preprocessing)
    if (/^<div\s/.test(line.trim())) {
      let html = line + '\n';
      i++;
      let depth = 1;
      while (i < lines.length && depth > 0) {
        if (/<div[\s>]/.test(lines[i])) depth++;
        if (/<\/div>/.test(lines[i])) depth--;
        html += lines[i] + '\n';
        i++;
      }
      blocks.push({ type: 'html', html });
      continue;
    }

    // Fenced code block
    const fenceMatch = line.match(/^(`{3,}|~{3,})(\w*)/);
    if (fenceMatch) {
      const fence = fenceMatch[1];
      const lang = fenceMatch[2] || '';
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith(fence)) {
        codeLines.push(lines[i]);
        i++;
      }
      if (i < lines.length) i++; // skip closing fence
      blocks.push({ type: 'code', content: codeLines.join('\n'), lang });
      continue;
    }

    // ATX heading
    const headingMatch = line.match(/^(#{1,6})\s+(.+?)(?:\s+#+)?$/);
    if (headingMatch) {
      blocks.push({ type: 'heading', level: headingMatch[1].length, content: headingMatch[2] });
      i++;
      continue;
    }

    // Horizontal rule
    if (/^(?:[-*_]\s*){3,}$/.test(line.trim())) {
      blocks.push({ type: 'hr' });
      i++;
      continue;
    }

    // Table
    if (i + 1 < lines.length && /^\|/.test(line) && /^\|[\s:|-]+\|/.test(lines[i + 1])) {
      const headerCells = parseTableRow(line);
      const alignLine = lines[i + 1];
      const alignments = parseAlignments(alignLine);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && /^\|/.test(lines[i])) {
        rows.push(parseTableRow(lines[i]));
        i++;
      }
      blocks.push({ type: 'table', rows: [headerCells, ...rows], alignments });
      continue;
    }

    // Blockquote
    if (/^>\s?/.test(line)) {
      const quoteLines: string[] = [];
      while (i < lines.length && (/^>\s?/.test(lines[i]) || (/^\S/.test(lines[i]) && !/^#{1,6}\s/.test(lines[i]) && !/^[-*_]{3,}/.test(lines[i])))) {
        quoteLines.push(lines[i].replace(/^>\s?/, ''));
        i++;
      }
      const innerBlocks = parseBlocks(quoteLines.join('\n'));
      blocks.push({ type: 'blockquote', html: renderBlocks(innerBlocks) });
      continue;
    }

    // Unordered list
    if (/^(\s*)([-*+])\s/.test(line)) {
      const result = parseList(lines, i, false);
      blocks.push(result.block);
      i = result.nextIndex;
      continue;
    }

    // Ordered list
    if (/^(\s*)\d+[.)]\s/.test(line)) {
      const result = parseList(lines, i, true);
      blocks.push(result.block);
      i = result.nextIndex;
      continue;
    }

    // Setext heading (h1: ===, h2: ---)
    if (i + 1 < lines.length) {
      if (/^={2,}\s*$/.test(lines[i + 1])) {
        blocks.push({ type: 'heading', level: 1, content: line.trim() });
        i += 2;
        continue;
      }
      if (/^-{2,}\s*$/.test(lines[i + 1]) && !/^[-*+]\s/.test(line)) {
        blocks.push({ type: 'heading', level: 2, content: line.trim() });
        i += 2;
        continue;
      }
    }

    // Paragraph (collect consecutive non-blank lines)
    const paraLines: string[] = [line];
    i++;
    while (i < lines.length && /\S/.test(lines[i]) &&
      !/^#{1,6}\s/.test(lines[i]) &&
      !/^(`{3,}|~{3,})/.test(lines[i]) &&
      !/^>\s?/.test(lines[i]) &&
      !/^[-*+]\s/.test(lines[i]) &&
      !/^\d+[.)]\s/.test(lines[i]) &&
      !/^(?:[-*_]\s*){3,}$/.test(lines[i].trim()) &&
      !/^\|/.test(lines[i]) &&
      !/^<div\s/.test(lines[i].trim())) {
      paraLines.push(lines[i]);
      i++;
    }
    blocks.push({ type: 'paragraph', content: paraLines.join('\n') });
  }

  return blocks;
}

function parseTableRow(line: string): string[] {
  return line.replace(/^\||\|$/g, '').split('|').map(c => c.trim());
}

function parseAlignments(line: string): string[] {
  return line.replace(/^\||\|$/g, '').split('|').map(c => {
    const t = c.trim();
    if (t.startsWith(':') && t.endsWith(':')) return 'center';
    if (t.endsWith(':')) return 'right';
    return 'left';
  });
}

function parseList(lines: string[], startIdx: number, ordered: boolean): { block: Block; nextIndex: number } {
  const items: ListItem[] = [];
  let i = startIdx;
  const itemPattern = ordered ? /^(\s*)\d+[.)]\s(.*)$/ : /^(\s*)[-*+]\s(.*)$/;
  const baseIndent = (lines[i].match(/^(\s*)/) || ['', ''])[1].length;

  while (i < lines.length) {
    const m = lines[i].match(itemPattern);
    if (!m) break;
    const indent = m[1].length;
    if (indent < baseIndent) break;
    if (indent > baseIndent) break;

    let itemText = m[2];
    let checked: boolean | null = null;

    // Task list checkbox
    const checkMatch = itemText.match(/^\[([xX ])\]\s*(.*)/);
    if (checkMatch) {
      checked = checkMatch[1].toLowerCase() === 'x';
      itemText = checkMatch[2];
    }

    // Collect continuation lines
    i++;
    while (i < lines.length && /^\s+\S/.test(lines[i]) && !itemPattern.test(lines[i])) {
      itemText += '\n' + lines[i].trim();
      i++;
    }

    items.push({ content: itemText, checked });
  }

  return {
    block: { type: 'list', items, ordered },
    nextIndex: i,
  };
}

// ── Rendering ──

function renderBlocks(blocks: Block[]): string {
  return blocks.map(renderBlock).join('\n');
}

function renderBlock(block: Block): string {
  switch (block.type) {
    case 'heading':
      return `<h${block.level}>${parseInline(block.content || '')}</h${block.level}>`;

    case 'paragraph':
      return `<p>${parseInline(block.content || '')}</p>`;

    case 'code':
      return highlightToHtml(block.content || '', block.lang || '');

    case 'hr':
      return '<hr />';

    case 'blockquote':
      return `<blockquote>${block.html || ''}</blockquote>`;

    case 'html':
      return block.html || '';

    case 'list': {
      const tag = block.ordered ? 'ol' : 'ul';
      const itemsHtml = (block.items || []).map(item => {
        const checkbox = item.checked !== null
          ? `<input type="checkbox" ${item.checked ? 'checked' : ''} disabled /> `
          : '';
        return `<li>${checkbox}${parseInline(item.content)}</li>`;
      }).join('\n');
      return `<${tag}>\n${itemsHtml}\n</${tag}>`;
    }

    case 'table': {
      const rows = block.rows || [];
      const aligns = block.alignments || [];
      if (rows.length === 0) return '';
      const [header, ...body] = rows;
      const thCells = header.map((cell, idx) => {
        const align = aligns[idx] ? ` style="text-align:${aligns[idx]}"` : '';
        return `<th${align}>${parseInline(cell)}</th>`;
      }).join('');
      const bodyRows = body.map(row => {
        const cells = row.map((cell, idx) => {
          const align = aligns[idx] ? ` style="text-align:${aligns[idx]}"` : '';
          return `<td${align}>${parseInline(cell)}</td>`;
        }).join('');
        return `<tr>${cells}</tr>`;
      }).join('\n');
      return `<table>\n<thead><tr>${thCells}</tr></thead>\n<tbody>\n${bodyRows}\n</tbody>\n</table>`;
    }

    default:
      return '';
  }
}

export function renderMarkdown(source: string): string {
  const blocks = parseBlocks(source);
  return renderBlocks(blocks);
}
