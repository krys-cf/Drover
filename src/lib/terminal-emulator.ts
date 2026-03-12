/**
 * Native terminal emulator — ANSI parser + virtual screen buffer.
 * No xterm.js. This is our own implementation.
 *
 * Supports:
 * - Cursor positioning (CUP, CUU, CUD, CUF, CUB, etc.)
 * - Screen clearing (ED, EL)
 * - SGR colors/styles (bold, underline, fg/bg 8-color, 256-color, truecolor)
 * - Alternate screen buffer (DECSET/DECRST 1049)
 * - Scrolling regions (DECSTBM)
 * - Line wrapping
 * - Insert/delete lines and characters
 * - Tab stops
 */

export interface CellStyle {
  fg: string;       // CSS color
  bg: string;       // CSS color
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
  inverse: boolean;
  strikethrough: boolean;
}

export interface Cell {
  char: string;
  style: CellStyle;
}

const DEFAULT_STYLE: CellStyle = Object.freeze({
  fg: '',
  bg: '',
  bold: false,
  dim: false,
  italic: false,
  underline: false,
  inverse: false,
  strikethrough: false,
});

function isDefaultStyle(s: CellStyle): boolean {
  return s === DEFAULT_STYLE || (
    s.fg === '' && s.bg === '' && !s.bold && !s.dim &&
    !s.italic && !s.underline && !s.inverse && !s.strikethrough
  );
}

// Standard 8 ANSI colors
const ANSI_COLORS = [
  '#000000', '#cc0000', '#4e9a06', '#c4a000',
  '#3465a4', '#75507b', '#06989a', '#d3d7cf',
];
// Bright variants
const ANSI_BRIGHT = [
  '#555753', '#ef2929', '#8ae234', '#fce94f',
  '#729fcf', '#ad7fa8', '#34e2e2', '#eeeeec',
];

function color256(n: number): string {
  if (n < 8) return ANSI_COLORS[n];
  if (n < 16) return ANSI_BRIGHT[n - 8];
  if (n < 232) {
    // 216 color cube: 16 + 36*r + 6*g + b
    const idx = n - 16;
    const b = (idx % 6) * 51;
    const g = (Math.floor(idx / 6) % 6) * 51;
    const r = Math.floor(idx / 36) * 51;
    return `rgb(${r},${g},${b})`;
  }
  // Grayscale: 232-255
  const v = (n - 232) * 10 + 8;
  return `rgb(${v},${v},${v})`;
}

export class TerminalEmulator {
  rows: number;
  cols: number;
  cursorRow = 0;
  cursorCol = 0;
  savedCursorRow = 0;
  savedCursorCol = 0;

  // Primary screen buffer
  private primaryBuffer: Cell[][];
  private primaryScrollback: Cell[][] = [];

  // Alternate screen buffer (for TUI apps)
  private altBuffer: Cell[][] | null = null;
  private altCursorRow = 0;
  private altCursorCol = 0;

  // Which buffer is active
  useAltBuffer = false;

  // Current style for new characters
  private currentStyle: CellStyle = { ...DEFAULT_STYLE };

  // Scroll region (top/bottom, 0-indexed)
  private scrollTop = 0;
  private scrollBottom: number;

  // Parser state
  private parseState: 'ground' | 'escape' | 'csi' | 'osc' | 'dcs' = 'ground';
  private escBuffer = '';
  private oscBuffer = '';

  // Callbacks
  onTuiModeChange?: (active: boolean) => void;
  onBell?: () => void;
  onTitleChange?: (title: string) => void;
  onCwdChange?: (cwd: string) => void;

  // Current terminal title (set by TUI apps via OSC)
  title = '';

  // Max scrollback lines
  private maxScrollback = 5000;

  // Mouse tracking modes (set by TUI apps via DECSET)
  mouseTrackingMode: 'none' | 'normal' | 'button' | 'any' = 'none';
  mouseSgrMode = false;  // SGR extended mouse encoding (1006)

  constructor(rows: number, cols: number) {
    this.rows = rows;
    this.cols = cols;
    this.scrollBottom = rows - 1;
    this.primaryBuffer = this.createBuffer(rows, cols);
  }

  private createBuffer(rows: number, cols: number): Cell[][] {
    const buf: Cell[][] = [];
    for (let r = 0; r < rows; r++) {
      buf.push(this.createRow(cols));
    }
    return buf;
  }

  private createRow(cols: number): Cell[] {
    const row: Cell[] = [];
    for (let c = 0; c < cols; c++) {
      row.push({ char: ' ', style: DEFAULT_STYLE });
    }
    return row;
  }

  get buffer(): Cell[][] {
    return this.useAltBuffer && this.altBuffer ? this.altBuffer : this.primaryBuffer;
  }

  get scrollback(): Cell[][] {
    return this.primaryScrollback;
  }

  resize(rows: number, cols: number) {
    this.rows = rows;
    this.cols = cols;
    this.scrollBottom = rows - 1;
    this.scrollTop = 0;

    // Resize primary buffer
    this.resizeBuffer(this.primaryBuffer, rows, cols);
    if (this.altBuffer) {
      this.resizeBuffer(this.altBuffer, rows, cols);
    }

    // Clamp cursor
    this.cursorRow = Math.min(this.cursorRow, rows - 1);
    this.cursorCol = Math.min(this.cursorCol, cols - 1);
  }

  private resizeBuffer(buf: Cell[][], rows: number, cols: number) {
    // Add/remove rows
    while (buf.length < rows) buf.push(this.createRow(cols));
    while (buf.length > rows) buf.pop();
    // Resize each row
    for (const row of buf) {
      while (row.length < cols) row.push({ char: ' ', style: DEFAULT_STYLE });
      while (row.length > cols) row.pop();
    }
  }

  /**
   * Feed raw PTY data into the emulator.
   */
  write(data: string) {
    for (let i = 0; i < data.length; i++) {
      const ch = data[i];
      const code = data.charCodeAt(i);

      switch (this.parseState) {
        case 'ground':
          if (code === 0x1b) {
            this.parseState = 'escape';
            this.escBuffer = '';
          } else if (code === 0x07) {
            // Bell
            this.onBell?.();
          } else if (code === 0x08) {
            // Backspace
            if (this.cursorCol > 0) this.cursorCol--;
          } else if (code === 0x09) {
            // Tab — move to next tab stop (every 8 cols)
            this.cursorCol = Math.min(this.cols - 1, (Math.floor(this.cursorCol / 8) + 1) * 8);
          } else if (ch === '\r') {
            this.cursorCol = 0;
          } else if (ch === '\n') {
            this.lineFeed();
          } else if (code >= 0x20) {
            this.putChar(ch);
          }
          break;

        case 'escape':
          if (ch === '[') {
            this.parseState = 'csi';
            this.escBuffer = '';
          } else if (ch === ']') {
            this.parseState = 'osc';
            this.oscBuffer = '';
          } else if (ch === 'P') {
            this.parseState = 'dcs';
            this.escBuffer = '';
          } else if (ch === '(') {
            // Designate character set — skip next char
            i++;
            this.parseState = 'ground';
          } else if (ch === ')' || ch === '*' || ch === '+') {
            i++;
            this.parseState = 'ground';
          } else if (ch === '7') {
            // Save cursor
            this.savedCursorRow = this.cursorRow;
            this.savedCursorCol = this.cursorCol;
            this.parseState = 'ground';
          } else if (ch === '8') {
            // Restore cursor
            this.cursorRow = this.savedCursorRow;
            this.cursorCol = this.savedCursorCol;
            this.parseState = 'ground';
          } else if (ch === 'D') {
            // Index — move cursor down, scroll if needed
            this.lineFeed();
            this.parseState = 'ground';
          } else if (ch === 'M') {
            // Reverse index — move cursor up, scroll if needed
            this.reverseIndex();
            this.parseState = 'ground';
          } else if (ch === 'E') {
            // Next line
            this.cursorCol = 0;
            this.lineFeed();
            this.parseState = 'ground';
          } else if (ch === 'c') {
            // Full reset
            this.reset();
            this.parseState = 'ground';
          } else if (ch === '=') {
            // Application keypad mode — ignore
            this.parseState = 'ground';
          } else if (ch === '>') {
            // Normal keypad mode — ignore
            this.parseState = 'ground';
          } else {
            // Unknown escape, ignore
            this.parseState = 'ground';
          }
          break;

        case 'csi':
          if ((code >= 0x30 && code <= 0x3f) || ch === ';' || ch === ' ') {
            // Parameter bytes and intermediates
            this.escBuffer += ch;
          } else if (code >= 0x40 && code <= 0x7e) {
            // Final byte — dispatch
            this.handleCSI(this.escBuffer, ch);
            this.parseState = 'ground';
          } else {
            // Invalid, abort
            this.parseState = 'ground';
          }
          break;

        case 'osc':
          if (code === 0x07 || (ch === '\\' && this.oscBuffer.endsWith('\x1b'))) {
            // OSC terminated by BEL or ST
            if (ch === '\\') {
              this.oscBuffer = this.oscBuffer.slice(0, -1); // remove trailing ESC
            }
            this.handleOSC(this.oscBuffer);
            this.parseState = 'ground';
          } else {
            this.oscBuffer += ch;
          }
          break;

        case 'dcs':
          // Device Control String — skip until ST
          if (ch === '\\' && this.escBuffer.endsWith('\x1b')) {
            this.parseState = 'ground';
          } else {
            this.escBuffer += ch;
          }
          break;
      }
    }
  }

  private putChar(ch: string) {
    const buf = this.buffer;
    if (this.cursorCol >= this.cols) {
      // Wrap to next line
      this.cursorCol = 0;
      this.lineFeed();
    }
    if (this.cursorRow >= 0 && this.cursorRow < this.rows) {
      buf[this.cursorRow][this.cursorCol] = {
        char: ch,
        style: isDefaultStyle(this.currentStyle) ? DEFAULT_STYLE : { ...this.currentStyle },
      };
      this.cursorCol++;
    }
  }

  private lineFeed() {
    if (this.cursorRow === this.scrollBottom) {
      this.scrollUp();
    } else if (this.cursorRow < this.rows - 1) {
      this.cursorRow++;
    }
  }

  private reverseIndex() {
    if (this.cursorRow === this.scrollTop) {
      this.scrollDown();
    } else if (this.cursorRow > 0) {
      this.cursorRow--;
    }
  }

  private scrollUp() {
    const buf = this.buffer;
    // Move top line of scroll region to scrollback (primary buffer only)
    if (!this.useAltBuffer && this.scrollTop === 0) {
      this.primaryScrollback.push(buf[this.scrollTop]);
      if (this.primaryScrollback.length > this.maxScrollback) {
        this.primaryScrollback.shift();
      }
    }
    // Shift lines up within scroll region
    for (let r = this.scrollTop; r < this.scrollBottom; r++) {
      buf[r] = buf[r + 1];
    }
    buf[this.scrollBottom] = this.createRow(this.cols);
  }

  private scrollDown() {
    const buf = this.buffer;
    // Shift lines down within scroll region
    for (let r = this.scrollBottom; r > this.scrollTop; r--) {
      buf[r] = buf[r - 1];
    }
    buf[this.scrollTop] = this.createRow(this.cols);
  }

  private handleCSI(params: string, final: string) {
    // Parse params like "1;2" or "?25" etc.
    const isPrivate = params.startsWith('?');
    const paramStr = isPrivate ? params.slice(1) : params;
    const parts = paramStr.split(';').map(s => parseInt(s, 10) || 0);

    switch (final) {
      case 'A': // Cursor Up
        this.cursorRow = Math.max(this.scrollTop, this.cursorRow - Math.max(1, parts[0]));
        break;
      case 'B': // Cursor Down
        this.cursorRow = Math.min(this.scrollBottom, this.cursorRow + Math.max(1, parts[0]));
        break;
      case 'C': // Cursor Forward
        this.cursorCol = Math.min(this.cols - 1, this.cursorCol + Math.max(1, parts[0]));
        break;
      case 'D': // Cursor Backward
        this.cursorCol = Math.max(0, this.cursorCol - Math.max(1, parts[0]));
        break;
      case 'E': // Cursor Next Line
        this.cursorCol = 0;
        this.cursorRow = Math.min(this.scrollBottom, this.cursorRow + Math.max(1, parts[0]));
        break;
      case 'F': // Cursor Previous Line
        this.cursorCol = 0;
        this.cursorRow = Math.max(this.scrollTop, this.cursorRow - Math.max(1, parts[0]));
        break;
      case 'G': // Cursor Horizontal Absolute
        this.cursorCol = Math.min(this.cols - 1, Math.max(0, (parts[0] || 1) - 1));
        break;
      case 'H': // Cursor Position
      case 'f': // Horizontal and Vertical Position
        this.cursorRow = Math.min(this.rows - 1, Math.max(0, (parts[0] || 1) - 1));
        this.cursorCol = Math.min(this.cols - 1, Math.max(0, (parts[1] || 1) - 1));
        break;
      case 'J': // Erase in Display
        this.eraseInDisplay(parts[0] || 0);
        break;
      case 'K': // Erase in Line
        this.eraseInLine(parts[0] || 0);
        break;
      case 'L': // Insert Lines
        this.insertLines(Math.max(1, parts[0]));
        break;
      case 'M': // Delete Lines
        this.deleteLines(Math.max(1, parts[0]));
        break;
      case 'P': // Delete Characters
        this.deleteChars(Math.max(1, parts[0]));
        break;
      case '@': // Insert Characters
        this.insertChars(Math.max(1, parts[0]));
        break;
      case 'S': // Scroll Up
        for (let i = 0; i < Math.max(1, parts[0]); i++) this.scrollUp();
        break;
      case 'T': // Scroll Down
        for (let i = 0; i < Math.max(1, parts[0]); i++) this.scrollDown();
        break;
      case 'd': // Vertical Position Absolute
        this.cursorRow = Math.min(this.rows - 1, Math.max(0, (parts[0] || 1) - 1));
        break;
      case 'm': // SGR — Select Graphic Rendition
        this.handleSGR(paramStr);
        break;
      case 'r': // Set Scrolling Region (DECSTBM)
        this.scrollTop = Math.max(0, (parts[0] || 1) - 1);
        this.scrollBottom = Math.min(this.rows - 1, (parts[1] || this.rows) - 1);
        this.cursorRow = 0;
        this.cursorCol = 0;
        break;
      case 'h': // Set Mode
        if (isPrivate) this.handleDECSET(parts);
        break;
      case 'l': // Reset Mode
        if (isPrivate) this.handleDECRST(parts);
        break;
      case 's': // Save Cursor Position
        this.savedCursorRow = this.cursorRow;
        this.savedCursorCol = this.cursorCol;
        break;
      case 'u': // Restore Cursor Position
        this.cursorRow = this.savedCursorRow;
        this.cursorCol = this.savedCursorCol;
        break;
      case 'X': // Erase Characters
        this.eraseChars(Math.max(1, parts[0]));
        break;
      case 'n': // Device Status Report
        // We don't respond to these, ignore
        break;
      case 'c': // Device Attributes
        // Ignore
        break;
      case 't': // Window manipulation — ignore
        break;
      case 'q': // Set cursor style — ignore for now
        break;
      default:
        // Unknown CSI, ignore
        break;
    }
  }

  private handleSGR(paramStr: string) {
    if (!paramStr || paramStr === '0') {
      this.currentStyle = DEFAULT_STYLE;
      return;
    }

    const parts = paramStr.split(';').map(s => parseInt(s, 10));
    let i = 0;
    while (i < parts.length) {
      const p = isNaN(parts[i]) ? 0 : parts[i];
      // Copy-on-write: ensure mutable copy before any mutation
      if (p !== 0 && this.currentStyle === DEFAULT_STYLE) {
        this.currentStyle = { ...DEFAULT_STYLE };
      }
      switch (p) {
        case 0: this.currentStyle = DEFAULT_STYLE; break;
        case 1: this.currentStyle.bold = true; break;
        case 2: this.currentStyle.dim = true; break;
        case 3: this.currentStyle.italic = true; break;
        case 4: this.currentStyle.underline = true; break;
        case 7: this.currentStyle.inverse = true; break;
        case 9: this.currentStyle.strikethrough = true; break;
        case 21: this.currentStyle.bold = false; break;
        case 22: this.currentStyle.bold = false; this.currentStyle.dim = false; break;
        case 23: this.currentStyle.italic = false; break;
        case 24: this.currentStyle.underline = false; break;
        case 27: this.currentStyle.inverse = false; break;
        case 29: this.currentStyle.strikethrough = false; break;
        // Foreground colors
        case 30: case 31: case 32: case 33:
        case 34: case 35: case 36: case 37:
          this.currentStyle.fg = ANSI_COLORS[p - 30]; break;
        case 38:
          // Extended foreground
          if (parts[i + 1] === 5 && parts[i + 2] !== undefined) {
            this.currentStyle.fg = color256(parts[i + 2]);
            i += 2;
          } else if (parts[i + 1] === 2 && parts[i + 4] !== undefined) {
            this.currentStyle.fg = `rgb(${parts[i + 2]},${parts[i + 3]},${parts[i + 4]})`;
            i += 4;
          }
          break;
        case 39: this.currentStyle.fg = ''; break;
        // Background colors
        case 40: case 41: case 42: case 43:
        case 44: case 45: case 46: case 47:
          this.currentStyle.bg = ANSI_COLORS[p - 40]; break;
        case 48:
          // Extended background
          if (parts[i + 1] === 5 && parts[i + 2] !== undefined) {
            this.currentStyle.bg = color256(parts[i + 2]);
            i += 2;
          } else if (parts[i + 1] === 2 && parts[i + 4] !== undefined) {
            this.currentStyle.bg = `rgb(${parts[i + 2]},${parts[i + 3]},${parts[i + 4]})`;
            i += 4;
          }
          break;
        case 49: this.currentStyle.bg = ''; break;
        // Bright foreground
        case 90: case 91: case 92: case 93:
        case 94: case 95: case 96: case 97:
          this.currentStyle.fg = ANSI_BRIGHT[p - 90]; break;
        // Bright background
        case 100: case 101: case 102: case 103:
        case 104: case 105: case 106: case 107:
          this.currentStyle.bg = ANSI_BRIGHT[p - 100]; break;
      }
      i++;
    }
  }

  private handleDECSET(params: number[]) {
    for (const p of params) {
      switch (p) {
        case 1049: // Alternate screen buffer
        case 47:
        case 1047:
          if (!this.useAltBuffer) {
            this.altCursorRow = this.cursorRow;
            this.altCursorCol = this.cursorCol;
            this.altBuffer = this.createBuffer(this.rows, this.cols);
            this.useAltBuffer = true;
            this.cursorRow = 0;
            this.cursorCol = 0;
            this.scrollTop = 0;
            this.scrollBottom = this.rows - 1;
            this.onTuiModeChange?.(true);
          }
          break;
        case 25: // Show cursor — ignore for now
          break;
        case 1: // Application cursor keys — we handle in key mapping
          break;
        case 7: // Auto-wrap — already default
          break;
        case 12: // Start blinking cursor — ignore
          break;
        case 1000: // Normal mouse tracking
          this.mouseTrackingMode = 'normal';
          break;
        case 1002: // Button-event mouse tracking
          this.mouseTrackingMode = 'button';
          break;
        case 1003: // Any-event mouse tracking
          this.mouseTrackingMode = 'any';
          break;
        case 1006: // SGR extended mouse encoding
          this.mouseSgrMode = true;
          break;
        case 2004: // Bracketed paste mode — ignore for now
          break;
      }
    }
  }

  private handleDECRST(params: number[]) {
    for (const p of params) {
      switch (p) {
        case 1049: // Leave alternate screen buffer
        case 47:
        case 1047:
          if (this.useAltBuffer) {
            this.useAltBuffer = false;
            this.altBuffer = null;
            this.cursorRow = this.altCursorRow;
            this.cursorCol = this.altCursorCol;
            this.scrollTop = 0;
            this.scrollBottom = this.rows - 1;
            this.mouseTrackingMode = 'none';
            this.mouseSgrMode = false;
            this.onTuiModeChange?.(false);
          }
          break;
        case 25: // Hide cursor — ignore for now
          break;
        case 1: // Normal cursor keys
          break;
        case 1000:
        case 1002:
        case 1003:
          this.mouseTrackingMode = 'none';
          break;
        case 1006:
          this.mouseSgrMode = false;
          break;
        case 2004: // Disable bracketed paste
          break;
      }
    }
  }

  private handleOSC(data: string) {
    const idx = data.indexOf(';');
    if (idx === -1) return;
    const code = parseInt(data.substring(0, idx), 10);
    const value = data.substring(idx + 1);
    switch (code) {
      case 0: // Set icon name and window title
      case 2: // Set window title
        this.title = value;
        this.onTitleChange?.(value);
        break;
      case 7: // Current working directory (file://host/path)
        try {
          const url = new URL(value);
          if (url.protocol === 'file:') {
            let path = decodeURIComponent(url.pathname);
            // On Windows, pathname starts with /C:/... — strip leading slash
            if (/^\/[A-Za-z]:\//.test(path)) path = path.substring(1);
            this.onCwdChange?.(path);
          }
        } catch { /* ignore malformed URLs */ }
        break;
    }
  }

  private eraseInDisplay(mode: number) {
    const buf = this.buffer;
    switch (mode) {
      case 0: // Erase from cursor to end
        this.eraseInLine(0);
        for (let r = this.cursorRow + 1; r < this.rows; r++) {
          buf[r] = this.createRow(this.cols);
        }
        break;
      case 1: // Erase from start to cursor
        for (let r = 0; r < this.cursorRow; r++) {
          buf[r] = this.createRow(this.cols);
        }
        for (let c = 0; c <= this.cursorCol && c < this.cols; c++) {
          buf[this.cursorRow][c] = { char: ' ', style: DEFAULT_STYLE };
        }
        break;
      case 2: // Erase entire display
      case 3: // Erase display + scrollback
        for (let r = 0; r < this.rows; r++) {
          buf[r] = this.createRow(this.cols);
        }
        if (mode === 3) {
          this.primaryScrollback.length = 0;
        }
        break;
    }
  }

  private eraseInLine(mode: number) {
    const buf = this.buffer;
    const row = buf[this.cursorRow];
    if (!row) return;
    switch (mode) {
      case 0: // Erase from cursor to end of line
        for (let c = this.cursorCol; c < this.cols; c++) {
          row[c] = { char: ' ', style: DEFAULT_STYLE };
        }
        break;
      case 1: // Erase from start to cursor
        for (let c = 0; c <= this.cursorCol && c < this.cols; c++) {
          row[c] = { char: ' ', style: DEFAULT_STYLE };
        }
        break;
      case 2: // Erase entire line
        for (let c = 0; c < this.cols; c++) {
          row[c] = { char: ' ', style: DEFAULT_STYLE };
        }
        break;
    }
  }

  private eraseChars(count: number) {
    const buf = this.buffer;
    const row = buf[this.cursorRow];
    if (!row) return;
    for (let c = this.cursorCol; c < Math.min(this.cursorCol + count, this.cols); c++) {
      row[c] = { char: ' ', style: DEFAULT_STYLE };
    }
  }

  private insertLines(count: number) {
    const buf = this.buffer;
    for (let i = 0; i < count; i++) {
      // Shift lines down from cursor to scrollBottom
      for (let r = this.scrollBottom; r > this.cursorRow; r--) {
        buf[r] = buf[r - 1];
      }
      buf[this.cursorRow] = this.createRow(this.cols);
    }
    this.cursorCol = 0;
  }

  private deleteLines(count: number) {
    const buf = this.buffer;
    for (let i = 0; i < count; i++) {
      // Shift lines up from cursor to scrollBottom
      for (let r = this.cursorRow; r < this.scrollBottom; r++) {
        buf[r] = buf[r + 1];
      }
      buf[this.scrollBottom] = this.createRow(this.cols);
    }
    this.cursorCol = 0;
  }

  private deleteChars(count: number) {
    const buf = this.buffer;
    const row = buf[this.cursorRow];
    if (!row) return;
    row.splice(this.cursorCol, count);
    while (row.length < this.cols) {
      row.push({ char: ' ', style: DEFAULT_STYLE });
    }
  }

  private insertChars(count: number) {
    const buf = this.buffer;
    const row = buf[this.cursorRow];
    if (!row) return;
    const blanks: Cell[] = [];
    for (let i = 0; i < count; i++) {
      blanks.push({ char: ' ', style: DEFAULT_STYLE });
    }
    row.splice(this.cursorCol, 0, ...blanks);
    while (row.length > this.cols) row.pop();
  }

  reset() {
    this.currentStyle = DEFAULT_STYLE;
    this.cursorRow = 0;
    this.cursorCol = 0;
    this.scrollTop = 0;
    this.scrollBottom = this.rows - 1;
    this.useAltBuffer = false;
    this.altBuffer = null;
    for (let r = 0; r < this.rows; r++) {
      this.primaryBuffer[r] = this.createRow(this.cols);
    }
    this.primaryScrollback.length = 0;
  }

  /**
   * Get the visible lines as an array of styled spans for rendering.
   * Each line is an array of { text, style } runs.
   */
  getLines(): { text: string; style: CellStyle }[][] {
    const buf = this.buffer;
    const lines: { text: string; style: CellStyle }[][] = [];

    for (let r = 0; r < this.rows; r++) {
      const row = buf[r];
      if (!row) continue;
      const runs: { text: string; style: CellStyle }[] = [];
      let currentRun: { text: string; style: CellStyle } | null = null;

      for (let c = 0; c < this.cols; c++) {
        const cell = row[c];
        if (currentRun && stylesEqual(currentRun.style, cell.style)) {
          currentRun.text += cell.char;
        } else {
          if (currentRun) runs.push(currentRun);
          currentRun = { text: cell.char, style: { ...cell.style } };
        }
      }
      if (currentRun) runs.push(currentRun);
      lines.push(runs);
    }
    return lines;
  }

  /**
   * Get scrollback + visible buffer as plain text lines (for normal mode rendering).
   * Trims trailing whitespace from each line.
   */
  getScrollbackLines(): { text: string; style: CellStyle }[][] {
    const lines: { text: string; style: CellStyle }[][] = [];

    // Scrollback
    for (const row of this.primaryScrollback) {
      lines.push(rowToRuns(row, this.cols));
    }

    // Current visible buffer
    for (let r = 0; r < this.rows; r++) {
      const row = this.primaryBuffer[r];
      if (row) lines.push(rowToRuns(row, this.cols));
    }

    // Trim trailing empty rows so content builds from bottom up
    while (lines.length > 0) {
      const last = lines[lines.length - 1];
      const isEmpty = last.length === 0 || (last.length === 1 && last[0].text.trim() === '');
      if (isEmpty) lines.pop();
      else break;
    }

    return lines;
  }
}

function stylesEqual(a: CellStyle, b: CellStyle): boolean {
  return a.fg === b.fg && a.bg === b.bg &&
    a.bold === b.bold && a.dim === b.dim &&
    a.italic === b.italic && a.underline === b.underline &&
    a.inverse === b.inverse && a.strikethrough === b.strikethrough;
}

function rowToRuns(row: Cell[], cols: number): { text: string; style: CellStyle }[] {
  const runs: { text: string; style: CellStyle }[] = [];
  let currentRun: { text: string; style: CellStyle } | null = null;

  for (let c = 0; c < cols && c < row.length; c++) {
    const cell = row[c];
    if (currentRun && stylesEqual(currentRun.style, cell.style)) {
      currentRun.text += cell.char;
    } else {
      if (currentRun) runs.push(currentRun);
      currentRun = { text: cell.char, style: { ...cell.style } };
    }
  }
  if (currentRun) runs.push(currentRun);

  // Trim trailing spaces from last run
  if (runs.length > 0) {
    const last = runs[runs.length - 1];
    last.text = last.text.replace(/\s+$/, '');
    if (last.text === '' && runs.length > 1) runs.pop();
  }

  return runs;
}

/**
 * Encode a mouse event into the terminal escape sequence the PTY expects.
 * button: 0=left, 1=middle, 2=right, 64=scrollUp, 65=scrollDown, 3=release
 * col/row: 0-indexed cell coordinates
 * press: true for press, false for release
 */
export function encodeMouseEvent(
  emu: TerminalEmulator,
  button: number,
  col: number,
  row: number,
  press: boolean
): string | null {
  if (emu.mouseTrackingMode === 'none') return null;

  col = Math.max(0, Math.min(col, emu.cols - 1));
  row = Math.max(0, Math.min(row, emu.rows - 1));

  if (emu.mouseSgrMode) {
    // SGR encoding: \x1b[<button;col+1;row+1M (press) or m (release)
    return `\x1b[<${button};${col + 1};${row + 1}${press ? 'M' : 'm'}`;
  }

  // Legacy X10 encoding: \x1b[M cb cx cy (all + 32)
  const cb = button + 32;
  const cx = col + 1 + 32;
  const cy = row + 1 + 32;
  return `\x1b[M${String.fromCharCode(cb)}${String.fromCharCode(cx)}${String.fromCharCode(cy)}`;
}

/**
 * Map browser keyboard events to terminal escape sequences.
 */
export function keyToSequence(e: KeyboardEvent): string | null {
  // Ctrl+key combos
  if (e.ctrlKey && !e.metaKey && !e.altKey) {
    if (e.key.length === 1) {
      const code = e.key.toUpperCase().charCodeAt(0);
      if (code >= 65 && code <= 90) {
        return String.fromCharCode(code - 64);
      }
    }
  }

  // Alt+key (send as ESC + key)
  if (e.altKey && !e.ctrlKey && !e.metaKey && e.key.length === 1) {
    return '\x1b' + e.key;
  }

  switch (e.key) {
    case 'Enter': return e.shiftKey ? '\n' : '\r';
    case 'Backspace': return '\x7f';
    case 'Tab': return '\t';
    case 'Escape': return '\x1b';
    case 'ArrowUp': return '\x1b[A';
    case 'ArrowDown': return '\x1b[B';
    case 'ArrowRight': return '\x1b[C';
    case 'ArrowLeft': return '\x1b[D';
    case 'Home': return '\x1b[H';
    case 'End': return '\x1b[F';
    case 'Insert': return '\x1b[2~';
    case 'Delete': return '\x1b[3~';
    case 'PageUp': return '\x1b[5~';
    case 'PageDown': return '\x1b[6~';
    case 'F1': return '\x1bOP';
    case 'F2': return '\x1bOQ';
    case 'F3': return '\x1bOR';
    case 'F4': return '\x1bOS';
    case 'F5': return '\x1b[15~';
    case 'F6': return '\x1b[17~';
    case 'F7': return '\x1b[18~';
    case 'F8': return '\x1b[19~';
    case 'F9': return '\x1b[20~';
    case 'F10': return '\x1b[21~';
    case 'F11': return '\x1b[23~';
    case 'F12': return '\x1b[24~';
    default:
      // Regular printable character
      if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
        return e.key;
      }
      return null;
  }
}
