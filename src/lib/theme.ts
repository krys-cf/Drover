export interface TerminalTheme {
  id: string;
  name: string;
  promptUser: string;
  promptHost: string;
  promptPath: string;
  promptSymbol: string;
  promptCommand: string;
  directory: string;
  executable: string;
  symlink: string;
}

export const THEMES: Record<string, TerminalTheme> = {
  dracula: {
    id: 'dracula', name: 'Dracula',
    promptUser: '#bd93f9', promptHost: '#8be9fd', promptPath: '#50fa7b',
    promptSymbol: '#ff79c6', promptCommand: '#f8f8f2',
    directory: '#8be9fd', executable: '#50fa7b', symlink: '#bd93f9',
  },
  slate: {
    id: 'slate', name: 'Slate',
    promptUser: '#7aa2f7', promptHost: '#9ece6a', promptPath: '#e0af68',
    promptSymbol: '#bb9af7', promptCommand: '#c0caf5',
    directory: '#7aa2f7', executable: '#9ece6a', symlink: '#bb9af7',
  },
  custom: {
    id: 'custom', name: 'Custom',
    promptUser: '#bd93f9', promptHost: '#8be9fd', promptPath: '#50fa7b',
    promptSymbol: '#ff79c6', promptCommand: '#f8f8f2',
    directory: '#8be9fd', executable: '#50fa7b', symlink: '#bd93f9',
  },
};

export function applyThemeCssVars(theme: TerminalTheme) {
  const root = document.documentElement;
  root.style.setProperty('--prompt-user', theme.promptUser);
  root.style.setProperty('--prompt-host', theme.promptHost);
  root.style.setProperty('--prompt-path', theme.promptPath);
  root.style.setProperty('--prompt-symbol', theme.promptSymbol);
  root.style.setProperty('--prompt-command', theme.promptCommand);
  root.style.setProperty('--theme-directory', theme.directory);
  root.style.setProperty('--theme-executable', theme.executable);
  root.style.setProperty('--theme-symlink', theme.symlink);
}

export function highlightPrompt(lineText: string, theme: TerminalTheme): { text: string; color: string }[] | null {
  const match = lineText.match(/^(\S+)(@)(\S+)\s+(.+?)\s+(%)\s*(.*)$/);
  if (!match) return null;
  return [
    { text: match[1], color: theme.promptUser },
    { text: match[2], color: theme.promptHost },
    { text: match[3], color: theme.promptHost },
    { text: ' ', color: '' },
    { text: match[4], color: theme.promptPath },
    { text: ' ', color: '' },
    { text: match[5], color: theme.promptSymbol },
    { text: ' ', color: '' },
    { text: match[6], color: theme.promptCommand },
  ];
}

// ── Semantic output highlighting ──

type Seg = { text: string; color: string };

const C = {
  // Semantic colors (Dracula-inspired, theme-independent)
  red:      '#ff5555',
  orange:   '#ffb86c',
  yellow:   '#f1fa8c',
  green:    '#50fa7b',
  cyan:     '#8be9fd',
  purple:   '#bd93f9',
  pink:     '#ff79c6',
  white:    '#f8f8f2',
  dim:      '#6272a4',
  dimmer:   '#44475a',
};

/** Highlight a plain-text terminal output line with semantic colors.
 *  Returns null if no patterns match (caller renders default). */
export function highlightOutput(line: string): Seg[] | null {
  const trimmed = line.trimStart();
  if (!trimmed) return null;

  const indent = line.slice(0, line.length - trimmed.length);
  const indentSeg: Seg[] = indent ? [{ text: indent, color: '' }] : [];

  // ── Curl verbose prefix lines (* info, > request, < response, { } data) ──
  if (/^[*><{}]\s/.test(trimmed)) {
    const prefix = trimmed[0];
    const rest = trimmed.slice(1);

    const prefixColor =
      prefix === '*' ? C.dim :
      prefix === '>' ? C.cyan :
      prefix === '<' ? C.green :
      C.dimmer; // { }

    // SSL certificate fields
    if (prefix === '*') {
      // Expired cert date detection
      const expireMatch = rest.match(/(\s*expire date:\s*)(.+)/i);
      if (expireMatch) {
        const dateStr = expireMatch[2].trim();
        const isExpired = isDateExpired(dateStr);
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: expireMatch[1], color: C.dim },
          { text: expireMatch[2], color: isExpired ? C.red : C.green },
        ];
      }

      // Start date
      const startMatch = rest.match(/(\s*start date:\s*)(.+)/i);
      if (startMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: startMatch[1], color: C.dim },
          { text: startMatch[2], color: C.cyan },
        ];
      }

      // Subject / issuer
      const certFieldMatch = rest.match(/(\s*(?:subject|issuer):\s*)(.+)/i);
      if (certFieldMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: certFieldMatch[1], color: C.dim },
          { text: certFieldMatch[2], color: C.purple },
        ];
      }

      // SSL certificate verify result
      if (/SSL certificate verify result/i.test(rest)) {
        const isOk = /ok/i.test(rest) && !/unable|error|fail/i.test(rest);
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: rest, color: isOk ? C.green : C.red },
        ];
      }

      // SSL connection line
      if (/SSL connection using/i.test(rest)) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: rest, color: C.cyan },
        ];
      }

      // ALPN
      if (/ALPN/i.test(rest)) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: rest, color: C.cyan },
        ];
      }

      // TLS handshake lines
      if (/TLS/.test(rest)) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: rest, color: C.dim },
        ];
      }

      // "Connected to" line — highlight IP
      const connMatch = rest.match(/(\s*Connected to\s+)(\S+)(\s+\()(\d+\.\d+\.\d+\.\d+)(\).+)/);
      if (connMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: connMatch[1], color: C.dim },
          { text: connMatch[2], color: C.white },
          { text: connMatch[3], color: C.dim },
          { text: connMatch[4], color: C.cyan },
          { text: connMatch[5], color: C.dim },
        ];
      }

      // "Trying IP:port..."
      const tryMatch = rest.match(/(\s*Trying\s+)(\d+\.\d+\.\d+\.\d+)(:\d+)(\.\.\.)/);
      if (tryMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: tryMatch[1], color: C.dim },
          { text: tryMatch[2], color: C.cyan },
          { text: tryMatch[3], color: C.purple },
          { text: tryMatch[4], color: C.dim },
        ];
      }

      // Default * line
      return [...indentSeg,
        { text: prefix, color: prefixColor },
        { text: rest, color: C.dim },
      ];
    }

    // > Request headers and < Response headers
    if (prefix === '>' || prefix === '<') {
      // HTTP status line: "< HTTP/1.1 200 OK" or "> GET / HTTP/1.1"
      const httpStatusMatch = rest.match(/(\s*HTTP\/\S+\s+)(\d{3})(\s+.*)?/);
      if (httpStatusMatch) {
        const code = parseInt(httpStatusMatch[2], 10);
        const codeColor = code < 300 ? C.green : code < 400 ? C.yellow : code < 500 ? C.orange : C.red;
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: httpStatusMatch[1], color: prefixColor },
          { text: httpStatusMatch[2], color: codeColor },
          ...(httpStatusMatch[3] ? [{ text: httpStatusMatch[3], color: codeColor }] : []),
        ];
      }

      // Request line: "> GET / HTTP/1.1"
      const reqMatch = rest.match(/(\s*)(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)(\s+)(\S+)(.*)/);
      if (reqMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: reqMatch[1], color: '' },
          { text: reqMatch[2], color: C.pink },
          { text: reqMatch[3], color: '' },
          { text: reqMatch[4], color: C.white },
          { text: reqMatch[5], color: C.dim },
        ];
      }

      // Header line: "< Content-Type: application/json"
      const headerMatch = rest.match(/(\s*)([A-Za-z][\w-]*)(:)(\s*)(.*)/);
      if (headerMatch) {
        return [...indentSeg,
          { text: prefix, color: prefixColor },
          { text: headerMatch[1], color: '' },
          { text: headerMatch[2], color: prefix === '<' ? C.green : C.cyan },
          { text: headerMatch[3], color: C.dim },
          { text: headerMatch[4], color: '' },
          { text: headerMatch[5], color: C.white },
        ];
      }

      return [...indentSeg,
        { text: prefix, color: prefixColor },
        { text: rest, color: prefixColor },
      ];
    }

    // { } data lines — very dim
    return [...indentSeg,
      { text: prefix, color: C.dimmer },
      { text: rest, color: C.dimmer },
    ];
  }

  // ── Section headers (=== TITLE === or ======= TITLE =======) ──
  const sectionMatch = trimmed.match(/^(=+\s*)(.+?)(\s*=+)$/);
  if (sectionMatch) {
    return [...indentSeg,
      { text: sectionMatch[1], color: C.dim },
      { text: sectionMatch[2], color: C.pink },
      { text: sectionMatch[3], color: C.dim },
    ];
  }

  // ── Key: Value lines (like "http_code:  200" or "DNS Lookup:  0.000040") ──
  const kvMatch = trimmed.match(/^([A-Za-z][\w\s]*?):\s{2,}(.+)$/);
  if (kvMatch) {
    const key = kvMatch[1];
    const value = kvMatch[2].trim();
    const valueColor = getValueColor(key, value);
    return [...indentSeg,
      { text: key, color: C.dim },
      { text: ': ', color: C.dimmer },
      { text: line.slice(line.indexOf(': ') + 2), color: valueColor },
    ];
  }

  // ── "finished in Xm Ys" lines ──
  if (/^finished in /i.test(trimmed)) {
    return [...indentSeg,
      { text: trimmed, color: C.dim },
    ];
  }

  // ── "job created!" lines ──
  if (/^job created/i.test(trimmed)) {
    return [...indentSeg,
      ...highlightInlinePatterns(trimmed),
    ];
  }

  // ── Lines with IPs, URLs, or other inline patterns ──
  const inlineResult = highlightInlinePatterns(trimmed);
  if (inlineResult.length > 1 || (inlineResult.length === 1 && inlineResult[0].color)) {
    return [...indentSeg, ...inlineResult];
  }

  // No patterns matched
  return null;
}

/** Highlight inline patterns: IPs, URLs, HTTP status codes */
function highlightInlinePatterns(text: string): Seg[] {
  // Match IPs, URLs, and quoted strings
  const pattern = /(\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(?::\d+)?\b)|(https?:\/\/\S+)|(\b[1-5]\d{2}\b)/g;
  const segs: Seg[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let hasMatch = false;

  while ((match = pattern.exec(text)) !== null) {
    hasMatch = true;
    if (match.index > lastIndex) {
      segs.push({ text: text.slice(lastIndex, match.index), color: '' });
    }
    if (match[1]) {
      // IP address
      segs.push({ text: match[1], color: C.cyan });
    } else if (match[2]) {
      // URL
      segs.push({ text: match[2], color: C.cyan });
    } else if (match[3]) {
      // HTTP status code
      const code = parseInt(match[3], 10);
      if (code >= 100 && code < 600) {
        const color = code < 300 ? C.green : code < 400 ? C.yellow : code < 500 ? C.orange : C.red;
        segs.push({ text: match[3], color });
      } else {
        segs.push({ text: match[3], color: '' });
      }
    }
    lastIndex = pattern.lastIndex;
  }

  if (!hasMatch) return [{ text, color: '' }];

  if (lastIndex < text.length) {
    segs.push({ text: text.slice(lastIndex), color: '' });
  }
  return segs;
}

/** Color a value based on its key context */
function getValueColor(key: string, value: string): string {
  const lk = key.toLowerCase().trim();

  // HTTP status code
  if (lk === 'http_code' || lk === 'status') {
    const code = parseInt(value, 10);
    if (!isNaN(code)) {
      if (code < 300) return C.green;
      if (code < 400) return C.yellow;
      if (code < 500) return C.orange;
      return C.red;
    }
  }

  // Timing values
  if (lk.includes('time') || lk.includes('ttfb') || lk.includes('lookup') ||
      lk.includes('connection') || lk.includes('handshake') || lk.includes('transfer') ||
      lk.includes('total') || lk === 'dns lookup' || lk === 'tcp connection' ||
      lk === 'tls handshake' || lk === 'pre transfer') {
    const num = parseFloat(value);
    if (!isNaN(num)) {
      if (num < 0.1) return C.green;
      if (num < 1.0) return C.yellow;
      if (num < 5.0) return C.orange;
      return C.red;
    }
  }

  // Counts
  if (lk === 'num_connects' || lk === 'num_redirects') {
    return C.purple;
  }

  return C.white;
}

/** Check if a date string is in the past */
function isDateExpired(dateStr: string): boolean {
  try {
    const d = new Date(dateStr);
    if (isNaN(d.getTime())) return false;
    return d.getTime() < Date.now();
  } catch {
    return false;
  }
}

export const EXT_TO_LANG: Record<string, string> = {
  ts: 'typescript', tsx: 'tsx', js: 'javascript', jsx: 'jsx',
  py: 'python', rb: 'ruby', rs: 'rust', go: 'go',
  java: 'java', c: 'c', cpp: 'cpp', h: 'c', hpp: 'cpp',
  cs: 'csharp', swift: 'swift', kt: 'kotlin',
  html: 'html', htm: 'html', css: 'css', scss: 'scss', less: 'less',
  json: 'json', yaml: 'yaml', yml: 'yaml', toml: 'toml',
  xml: 'xml', svg: 'xml',
  md: 'markdown', mdx: 'mdx',
  sh: 'bash', bash: 'bash', zsh: 'bash', fish: 'fish',
  sql: 'sql', graphql: 'graphql', gql: 'graphql',
  dockerfile: 'dockerfile', docker: 'dockerfile',
  svelte: 'svelte', vue: 'vue', astro: 'astro',
  php: 'php', lua: 'lua', zig: 'zig', nim: 'nim',
  makefile: 'makefile', cmake: 'cmake',
  lock: 'json', gitignore: 'ini', env: 'ini',
};

export function detectLang(filename: string): string {
  const lower = filename.toLowerCase();
  if (lower === 'makefile' || lower === 'dockerfile') return EXT_TO_LANG[lower] || 'text';
  const ext = lower.split('.').pop() || '';
  return EXT_TO_LANG[ext] || 'text';
}

export const FILE_COLOR_MAP: Record<string, string> = {
  // JavaScript / TypeScript — yellow / blue
  js: '#e6d16c', mjs: '#e6d16c', cjs: '#e6d16c', jsx: '#e6d16c',
  ts: '#519aba', tsx: '#519aba', mts: '#519aba',
  // Web
  html: '#e44d26', htm: '#e44d26',
  css: '#563d7c', scss: '#cd6799', less: '#563d7c',
  svg: '#ffb13b', vue: '#41b883', svelte: '#ff3e00', astro: '#ff5d01',
  // Data / Config
  json: '#cbcb41', yaml: '#cb171e', yml: '#cb171e', toml: '#9c4221',
  xml: '#e37933', env: '#ecd53f', ini: '#8888a0',
  // Markdown
  md: '#519aba', mdx: '#519aba',
  // Shell
  sh: '#89e051', bash: '#89e051', zsh: '#89e051', fish: '#89e051',
  // Rust / Go / C / C++
  rs: '#dea584', go: '#00add8',
  c: '#555555', cpp: '#f34b7d', h: '#555555', hpp: '#f34b7d',
  // Python / Ruby / PHP
  py: '#3572a5', rb: '#701516', php: '#4f5d95',
  // Java / Kotlin / Swift / C#
  java: '#b07219', kt: '#a97bff', swift: '#f05138', cs: '#178600',
  // Misc
  sql: '#e38c00', graphql: '#e535ab', gql: '#e535ab',
  lua: '#000080', zig: '#f7a41d', nim: '#ffe953',
  dockerfile: '#384d54', docker: '#384d54',
  makefile: '#427819', cmake: '#427819',
  // Lock / git
  lock: '#555555', gitignore: '#555555',
  // Images
  png: '#a074c4', jpg: '#a074c4', jpeg: '#a074c4', gif: '#a074c4',
  webp: '#a074c4', ico: '#a074c4', bmp: '#a074c4',
  // Docs
  txt: '#8888a0', log: '#8888a0', csv: '#89e051',
  pdf: '#e44d26',
};

export function getFileColor(filename: string): string {
  const lower = filename.toLowerCase();
  if (lower === 'makefile' || lower === 'dockerfile') return FILE_COLOR_MAP[lower] || '';
  const ext = lower.split('.').pop() || '';
  return FILE_COLOR_MAP[ext] || '';
}
