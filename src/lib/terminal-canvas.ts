/**
 * Canvas-based terminal renderer for TUI mode.
 * Renders the emulator buffer directly to an HTML5 Canvas,
 * with custom mouse-driven text selection (including rectangular select).
 */

import type { TerminalEmulator, CellStyle } from './terminal-emulator';

export interface TerminalCanvasOptions {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  /** Default foreground color (CSS) */
  defaultFg: string;
  /** Default background color (CSS) */
  defaultBg: string;
  /** Cursor color */
  cursorColor: string;
  /** Selection highlight color (with alpha) */
  selectionColor: string;
  /** Padding in pixels [top, right, bottom, left] */
  padding: [number, number, number, number];
}

interface SelectionCoord {
  row: number;
  col: number;
}

type SelectionMode = 'linear' | 'block';

export interface TerminalSelection {
  start: SelectionCoord;
  end: SelectionCoord;
  active: boolean;
  mode: SelectionMode;
}

const SOFT_BOUNDARY_CHARS = new Set([
  '│', '┃', '║', '┆', '┇', '┊', '┋', '╎', '╏', '▏', '▕',
  '█', '▌', '▐', '▊', '▋', '▉',
  '|',
]);

const MIN_BOUNDARY_ROWS = 3;

function stripBoundaryChars(line: string): string {
  let s = 0;
  let e = line.length;
  while (s < e && SOFT_BOUNDARY_CHARS.has(line[s])) s++;
  while (e > s && SOFT_BOUNDARY_CHARS.has(line[e - 1])) e--;
  return line.slice(s, e);
}

const DEFAULT_OPTIONS: TerminalCanvasOptions = {
  fontFamily: "'JetBrains Mono', 'Fira Code', 'SF Mono', 'Cascadia Code', 'Menlo', monospace",
  fontSize: 13,
  lineHeight: 1.2,
  defaultFg: '#e4e4ed',
  defaultBg: '#0a0a0f',
  cursorColor: '#e4e4ed',
  selectionColor: 'rgba(68, 138, 255, 0.35)',
  padding: [4, 8, 4, 8],
};

export class TerminalCanvas {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private emulator: TerminalEmulator;
  private opts: TerminalCanvasOptions;
  private dpr: number;

  // Measured character metrics
  charWidth = 0;
  charHeight = 0;
  private baselineOffset = 0;

  // Selection state
  selection: TerminalSelection = { start: { row: 0, col: 0 }, end: { row: 0, col: 0 }, active: false, mode: 'linear' };
  private selecting = false;
  private softBoundaryColumns: number[] = [];
  private softBoundaryOriginCol = 0;
  private frozenText: string | null = null;
  private scrollAccumTop: string[] = [];   // lines that scrolled off the top during drag
  private scrollAccumBottom: string[] = []; // lines that scrolled off the bottom during drag

  // Callback for forwarding mouse events to PTY (button, col, row, isRelease)
  onMouseInput: ((button: number, col: number, row: number, isRelease: boolean) => void) | null = null;

  // Animation frame
  private rafId = 0;
  private dirty = true;

  constructor(canvas: HTMLCanvasElement, emulator: TerminalEmulator, opts?: Partial<TerminalCanvasOptions>) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d', { alpha: false })!;
    this.emulator = emulator;
    this.opts = { ...DEFAULT_OPTIONS, ...opts };
    this.dpr = window.devicePixelRatio || 1;

    this.measureFont();
    this.setupEvents();
    this.startRenderLoop();
  }

  /** Update options (e.g. when theme changes) */
  updateOptions(opts: Partial<TerminalCanvasOptions>) {
    const fontChanged = opts.fontFamily !== undefined || opts.fontSize !== undefined || opts.lineHeight !== undefined;
    Object.assign(this.opts, opts);
    if (fontChanged) this.measureFont();
    this.markDirty();
  }

  /** Measure actual character dimensions from the configured font */
  private measureFont() {
    const { fontSize, fontFamily, lineHeight } = this.opts;
    const ctx = this.ctx;
    ctx.font = `${fontSize}px ${fontFamily}`;
    const metrics = ctx.measureText('W');
    this.charWidth = metrics.width;
    this.charHeight = Math.ceil(fontSize * lineHeight);
    // Baseline: distance from top of cell to text baseline
    this.baselineOffset = Math.ceil(fontSize * 0.85);
  }

  /** Get the total canvas size needed for the current terminal dimensions */
  getRequiredSize(): { width: number; height: number } {
    const [pt, pr, pb, pl] = this.opts.padding;
    return {
      width: pl + this.emulator.cols * this.charWidth + pr,
      height: pt + this.emulator.rows * this.charHeight + pb,
    };
  }

  /** Resize the canvas to fit the container element */
  fitToContainer(container: HTMLElement) {
    const rect = container.getBoundingClientRect();
    const w = rect.width;
    const h = rect.height;

    this.canvas.style.width = `${w}px`;
    this.canvas.style.height = `${h}px`;
    this.canvas.width = Math.round(w * this.dpr);
    this.canvas.height = Math.round(h * this.dpr);
    this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    this.markDirty();
  }

  /** Calculate terminal dimensions (rows/cols) from a pixel size */
  getDimensions(width: number, height: number): { rows: number; cols: number } {
    const [pt, pr, pb, pl] = this.opts.padding;
    const cols = Math.max(40, Math.floor((width - pl - pr) / this.charWidth));
    const rows = Math.max(10, Math.floor((height - pt - pb) / this.charHeight));
    return { rows, cols };
  }

  /** Convert pixel coordinates to terminal row/col */
  pixelToCell(clientX: number, clientY: number): { row: number; col: number } {
    const rect = this.canvas.getBoundingClientRect();
    const [pt, , , pl] = this.opts.padding;
    const x = clientX - rect.left - pl;
    const y = clientY - rect.top - pt;
    const col = Math.max(0, Math.min(this.emulator.cols - 1, Math.floor(x / this.charWidth)));
    const row = Math.max(0, Math.min(this.emulator.rows - 1, Math.floor(y / this.charHeight)));
    return { row, col };
  }

  /** Mark the canvas as needing a redraw */
  markDirty() {
    this.dirty = true;
  }

  /** Get selected text from the emulator buffer */
  getSelectedText(): string {
    if (!this.selection.active) return '';
    if (this.frozenText !== null) return this.frozenText;
    return this.extractSelectedText();
  }

  private extractSelectedText(): string {
    const buf = this.emulator.buffer;
    const lines: string[] = [];

    if (this.selection.mode === 'block') {
      const minRow = Math.min(this.selection.start.row, this.selection.end.row);
      const maxRow = Math.max(this.selection.start.row, this.selection.end.row);
      const minCol = Math.min(this.selection.start.col, this.selection.end.col);
      const maxCol = Math.max(this.selection.start.col, this.selection.end.col);

      for (let r = minRow; r <= maxRow; r++) {
        if (r >= buf.length) continue;
        const row = buf[r];
        let text = '';

        for (let c = minCol; c <= maxCol && c < row.length; c++) {
          text += row[c].char;
        }

        lines.push(stripBoundaryChars(text.replace(/\s+$/, '')));
      }

      return lines.join('\n').replace(/\n{3,}/g, '\n\n');
    }

    const allLines: string[] = [];

    if (this.scrollAccumTop.length > 0) {
      allLines.push(...this.scrollAccumTop);
    }

    const { start, end } = this.normalizeSelection();
    const bounds = this.softBoundaryColumns.length > 0
      ? this.getSoftBounds(this.softBoundaryOriginCol)
      : null;

    for (let r = start.row; r <= end.row; r++) {
      if (r >= buf.length) continue;
      const row = buf[r];
      let startCol = bounds ? bounds.left : 0;
      let endCol = bounds ? bounds.right + 1 : this.emulator.cols;

      if (r === start.row) startCol = Math.max(startCol, start.col);
      if (r === end.row) endCol = Math.min(endCol, end.col + 1);

      let text = '';
      for (let c = startCol; c < endCol && c < row.length; c++) {
        text += row[c].char;
      }
      allLines.push(stripBoundaryChars(text.replace(/\s+$/, '')));
    }

    if (this.scrollAccumBottom.length > 0) {
      allLines.push(...this.scrollAccumBottom);
    }

    return allLines.join('\n').replace(/\n{3,}/g, '\n\n');
  }

  /** Normalize selection so start is before end */
  private normalizeSelection(): { start: SelectionCoord; end: SelectionCoord } {
    const { start, end } = this.selection;
    if (start.row < end.row || (start.row === end.row && start.col <= end.col)) {
      return { start, end };
    }
    return { start: end, end: start };
  }

  /** Has an active non-empty selection or frozen text ready to copy */
  hasSelection(): boolean {
    if (this.frozenText) return true;
    if (!this.selection.active) return false;
    const { start, end } = this.selection;
    return start.row !== end.row || start.col !== end.col;
  }

  /** Clear the current selection */
  clearSelection() {
    this.selection.active = false;
    this.frozenText = null;
    this.scrollAccumTop = [];
    this.scrollAccumBottom = [];
    this.markDirty();
  }

  // ─── Soft Boundary Detection (iTerm2-style) ─────────

  private detectSoftBoundaries(): void {
    const buf = this.emulator.buffer;
    const rows = this.emulator.rows;
    const cols = this.emulator.cols;
    const boundarySet = new Set<number>();
    const visibleRows = Math.min(rows, buf.length);
    if (visibleRows < MIN_BOUNDARY_ROWS) { this.softBoundaryColumns = []; return; }

    for (let c = 0; c < cols; c++) {
      let consecutive = 0;
      for (let r = 0; r < visibleRows; r++) {
        const cell = buf[r]?.[c];
        if (cell && SOFT_BOUNDARY_CHARS.has(cell.char)) {
          consecutive++;
        } else {
          consecutive = 0;
        }
        if (consecutive >= MIN_BOUNDARY_ROWS) {
          boundarySet.add(c);
          break;
        }
      }
    }

    for (let c = 1; c < cols - 1; c++) {
      if (boundarySet.has(c)) continue;
      let transitionCount = 0;
      for (let r = 0; r < visibleRows; r++) {
        const cell = buf[r]?.[c];
        const left = buf[r]?.[c - 1];
        const right = buf[r]?.[c + 1];
        if (!cell || !left || !right) continue;
        const bg = cell.style.bg || '';
        const lbg = left.style.bg || '';
        const rbg = right.style.bg || '';
        if (bg !== lbg || bg !== rbg) transitionCount++;
      }
      if (transitionCount >= visibleRows * 0.6) {
        boundarySet.add(c);
      }
    }

    this.softBoundaryColumns = Array.from(boundarySet).sort((a, b) => a - b);
  }

  private getSoftBounds(col: number): { left: number; right: number } {
    const bounds = this.softBoundaryColumns;
    let left = 0;
    let right = this.emulator.cols - 1;

    for (const b of bounds) {
      if (b < col && b >= left) left = b + 1;
      if (b > col && b <= right) { right = b - 1; break; }
    }

    return { left, right };
  }

  // ─── Rendering ───────────────────────────────────────

  private startRenderLoop() {
    const loop = () => {
      if (this.dirty) {
        this.dirty = false;
        this.render();
      }
      this.rafId = requestAnimationFrame(loop);
    };
    this.rafId = requestAnimationFrame(loop);
  }

  private render() {
    const ctx = this.ctx;
    const { defaultFg, defaultBg, cursorColor, selectionColor, fontSize, fontFamily, padding } = this.opts;
    const [pt, , , pl] = padding;
    const buf = this.emulator.buffer;
    const cw = this.charWidth;
    const ch = this.charHeight;
    const cols = this.emulator.cols;
    const rows = this.emulator.rows;
    const canvasW = this.canvas.width / this.dpr;
    const canvasH = this.canvas.height / this.dpr;

    // Clear with default background
    ctx.fillStyle = defaultBg;
    ctx.fillRect(0, 0, canvasW, canvasH);

    // Normalize selection for highlight
    const sel = this.selection.active ? this.normalizeSelection() : null;

    // Render each row
    for (let r = 0; r < rows; r++) {
      const row = buf[r];
      if (!row) continue;
      const y = pt + r * ch;

      // Draw cell backgrounds and selection highlights
      for (let c = 0; c < cols; c++) {
        const cell = row[c];
        if (!cell) continue;
        const x = pl + c * cw;

        // Cell background
        let bg = '';
        if (cell.style.inverse) {
          bg = cell.style.fg || defaultFg;
        } else if (cell.style.bg) {
          bg = cell.style.bg;
        }

        if (bg) {
          ctx.fillStyle = bg;
          ctx.fillRect(x, y, cw, ch);
        }

        // Selection highlight
        if (sel && this.isCellSelected(r, c, sel)) {
          ctx.fillStyle = selectionColor;
          ctx.fillRect(x, y, cw, ch);
        }
      }

      // Draw text for the row (batch by style for performance)
      let runStart = 0;
      let currentFont = this.buildFont(row[0]?.style);
      ctx.font = currentFont;

      for (let c = 0; c < cols; c++) {
        const cell = row[c];
        if (!cell || cell.char === ' ') continue;

        const font = this.buildFont(cell.style);
        if (font !== currentFont) {
          currentFont = font;
          ctx.font = currentFont;
        }

        let fg: string;
        if (cell.style.inverse) {
          fg = cell.style.bg || defaultBg;
        } else {
          fg = cell.style.fg || defaultFg;
        }

        if (cell.style.dim) {
          ctx.globalAlpha = 0.6;
        }

        ctx.fillStyle = fg;
        const x = pl + c * cw;
        ctx.fillText(cell.char, x, y + this.baselineOffset);

        if (cell.style.dim) {
          ctx.globalAlpha = 1.0;
        }

        // Underline
        if (cell.style.underline) {
          ctx.strokeStyle = fg;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.moveTo(x, y + ch - 1.5);
          ctx.lineTo(x + cw, y + ch - 1.5);
          ctx.stroke();
        }

        // Strikethrough
        if (cell.style.strikethrough) {
          ctx.strokeStyle = fg;
          ctx.lineWidth = 1;
          ctx.beginPath();
          const strikeY = y + ch / 2;
          ctx.moveTo(x, strikeY);
          ctx.lineTo(x + cw, strikeY);
          ctx.stroke();
        }
      }
    }

    // Draw cursor
    const cr = this.emulator.cursorRow;
    const cc = this.emulator.cursorCol;
    if (cr >= 0 && cr < rows && cc >= 0 && cc < cols) {
      const cx = pl + cc * cw;
      const cy = pt + cr * ch;
      ctx.fillStyle = cursorColor;
      ctx.globalAlpha = 0.7;
      ctx.fillRect(cx, cy, cw, ch);
      ctx.globalAlpha = 1.0;

      // Draw the character under cursor with inverted color
      const cursorCell = buf[cr]?.[cc];
      if (cursorCell && cursorCell.char !== ' ') {
        ctx.font = this.buildFont(cursorCell.style);
        ctx.fillStyle = defaultBg;
        ctx.fillText(cursorCell.char, cx, cy + this.baselineOffset);
      }
    }
  }

  private buildFont(style: CellStyle | undefined): string {
    if (!style) return `${this.opts.fontSize}px ${this.opts.fontFamily}`;
    const weight = style.bold ? 'bold' : 'normal';
    const fontStyle = style.italic ? 'italic' : 'normal';
    return `${fontStyle} ${weight} ${this.opts.fontSize}px ${this.opts.fontFamily}`;
  }

  private isCellSelected(row: number, col: number, sel: { start: SelectionCoord; end: SelectionCoord }): boolean {
    if (this.selection.mode === 'block') {
      const minRow = Math.min(sel.start.row, sel.end.row);
      const maxRow = Math.max(sel.start.row, sel.end.row);
      const minCol = Math.min(sel.start.col, sel.end.col);
      const maxCol = Math.max(sel.start.col, sel.end.col);
      return row >= minRow && row <= maxRow && col >= minCol && col <= maxCol;
    }

    if (row < sel.start.row || row > sel.end.row) return false;

    const bounds = this.softBoundaryColumns.length > 0
      ? this.getSoftBounds(this.softBoundaryOriginCol)
      : null;

    if (bounds && (col < bounds.left || col > bounds.right)) return false;

    const rowData = this.emulator.buffer[row];
    let contentEnd = 0;
    if (rowData) {
      for (let c = rowData.length - 1; c >= 0; c--) {
        if (rowData[c] && rowData[c].char !== ' ' && rowData[c].char !== '') {
          contentEnd = c;
          break;
        }
      }
    }

    if (row === sel.start.row && row === sel.end.row) {
      return col >= sel.start.col && col <= Math.min(sel.end.col, contentEnd);
    }
    if (row === sel.start.row) return col >= sel.start.col && col <= contentEnd;
    if (row === sel.end.row) return col <= Math.min(sel.end.col, contentEnd);
    return col <= contentEnd;
  }

  /** Whether a drag selection is currently in progress */
  isSelecting(): boolean {
    return this.selecting;
  }

  /**
   * Extract text from a single buffer row within soft bounds.
   */
  private extractRowText(rowIdx: number): string {
    const buf = this.emulator.buffer;
    const row = buf[rowIdx];
    if (!row) return '';
    const bounds = this.softBoundaryColumns.length > 0
      ? this.getSoftBounds(this.softBoundaryOriginCol)
      : null;
    const startCol = bounds ? bounds.left : 0;
    const endCol = bounds ? bounds.right + 1 : this.emulator.cols;
    let text = '';
    for (let c = startCol; c < endCol && c < row.length; c++) {
      text += row[c].char;
    }
    return stripBoundaryChars(text.replace(/\s+$/, ''));
  }

  /**
   * Call when the TUI buffer scrolls (content changes).
   * During active drag: captures rows scrolling off-screen into accumulators
   * and adjusts selection anchors so the selection grows with the scroll.
   * After finalized selection: clears visual highlight, keeps frozenText for copy.
   */
  handleBufferScroll(scrollingDown: boolean, lineCount: number): void {
    if (this.selecting && this.selection.active) {
      const { start, end } = this.normalizeSelection();
      if (scrollingDown) {
        for (let i = 0; i < lineCount; i++) {
          const captureRow = start.row + i;
          if (captureRow <= end.row && captureRow >= 0 && captureRow < this.emulator.rows) {
            this.scrollAccumTop.push(this.extractRowText(captureRow));
          }
        }
        this.selection.start.row = Math.max(0, this.selection.start.row - lineCount);
        this.selection.end.row = Math.max(0, this.selection.end.row - lineCount);
      } else {
        for (let i = 0; i < lineCount; i++) {
          const captureRow = end.row - i;
          if (captureRow >= start.row && captureRow >= 0 && captureRow < this.emulator.rows) {
            this.scrollAccumBottom.unshift(this.extractRowText(captureRow));
          }
        }
        this.selection.start.row = Math.min(this.emulator.rows - 1, this.selection.start.row + lineCount);
        this.selection.end.row = Math.min(this.emulator.rows - 1, this.selection.end.row + lineCount);
      }
      this.markDirty();
      return;
    }

    if (this.selection.active) {
      this.selection.active = false;
      this.markDirty();
    }
  }

  // ─── Mouse Selection Events ──────────────────────────

  private setupEvents() {
    this.canvas.addEventListener('mousedown', this.onMouseDown);
    this.canvas.addEventListener('dblclick', this.onDoubleClick);
  }

  /**
   * Mouse handling strategy:
   * - mousedown: record the origin cell and start pixel position. Don't forward anything yet.
   * - mousemove: if the mouse has moved beyond a small threshold (3px), enter selection mode.
   *   Draw the selection highlight as the user drags.
   * - mouseup: if we never entered selection mode (i.e. it was just a click, no drag),
   *   forward a click event to the TUI app (if mouse tracking is on).
   *   If we did enter selection mode, finalize the selection and don't forward anything.
   *
   * This gives us the same behavior as a real terminal:
   *   - Click+drag always selects text
   *   - A plain click is forwarded to the TUI app for interaction
   */
  private onMouseDown = (e: MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();

    const originCell = this.pixelToCell(e.clientX, e.clientY);
    const mode: SelectionMode = e.altKey ? 'block' : 'linear';
    const originX = e.clientX;
    const originY = e.clientY;
    let dragging = false;
    const DRAG_THRESHOLD = 3; // pixels before we consider it a drag

    // Clear any existing selection on new mousedown
    this.selection.active = false;
    this.selecting = false;
    this.frozenText = null;
    this.scrollAccumTop = [];
    this.scrollAccumBottom = [];
    this.markDirty();

    const onMouseMove = (ev: MouseEvent) => {
      const dx = ev.clientX - originX;
      const dy = ev.clientY - originY;

      if (!dragging && (Math.abs(dx) > DRAG_THRESHOLD || Math.abs(dy) > DRAG_THRESHOLD)) {
        dragging = true;
        this.selecting = true;
        if (mode === 'linear') {
          this.detectSoftBoundaries();
          this.softBoundaryOriginCol = originCell.col;
        } else {
          this.softBoundaryColumns = [];
        }
        this.selection = { start: { ...originCell }, end: { ...originCell }, active: true, mode };
      }

      if (dragging) {
        const cell = this.pixelToCell(ev.clientX, ev.clientY);
        this.selection.end = { ...cell };
        this.markDirty();
      }
    };

    const onMouseUp = (ev: MouseEvent) => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);

      if (dragging) {
        // Finalize selection
        this.selecting = false;
        const cell = this.pixelToCell(ev.clientX, ev.clientY);
        this.selection.end = { ...cell };
        // If start and end are the same cell, clear it (micro-drag, not meaningful)
        if (this.selection.start.row === this.selection.end.row &&
            this.selection.start.col === this.selection.end.col) {
          this.selection.active = false;
          this.frozenText = null;
        } else {
          this.frozenText = this.extractSelectedText();
        }
        this.markDirty();
      } else {
        // It was a click (no drag) — forward to the TUI app if mouse tracking is on
        if (this.emulator.mouseTrackingMode !== 'none') {
          this.onMouseInput?.(0, originCell.col, originCell.row, false);
          this.onMouseInput?.(0, originCell.col, originCell.row, true);
        }
      }
    };

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
  };

  private onDoubleClick = (e: MouseEvent) => {
    const cell = this.pixelToCell(e.clientX, e.clientY);
    const buf = this.emulator.buffer;
    const row = buf[cell.row];
    if (!row) return;

    const maxCol = this.emulator.cols - 1;

    // Find word boundaries
    let startCol = cell.col;
    let endCol = cell.col;

    const isWordChar = (ch: string) => /\S/.test(ch);
    const charAt = (c: number) => row[c]?.char || ' ';

    while (startCol > 0 && isWordChar(charAt(startCol - 1))) startCol--;
    while (endCol < maxCol && isWordChar(charAt(endCol + 1))) endCol++;

    this.selection = { start: { row: cell.row, col: startCol }, end: { row: cell.row, col: endCol }, active: true, mode: 'linear' };
    this.frozenText = this.extractSelectedText();
    this.markDirty();
  };

  // ─── Lifecycle ───────────────────────────────────────

  dispose() {
    cancelAnimationFrame(this.rafId);
    this.canvas.removeEventListener('mousedown', this.onMouseDown);
    this.canvas.removeEventListener('dblclick', this.onDoubleClick);
  }
}
