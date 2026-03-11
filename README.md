# Drover

A modern agentic terminal built with Rust. Native terminal emulation — no xterm.js.

## Tech Stack

- **Tauri 2** — Rust-powered desktop shell, native macOS window with overlay titlebar
- **SvelteKit + Svelte 5** — TypeScript frontend (SSG via adapter-static)
- **Bun** — Package manager and task runner
- **Vanilla CSS** — Custom dark design tokens (shadcn-style neutral palette), no framework dependencies
- **portable-pty** — Cross-platform PTY for real shell interaction
- **Native Terminal Emulator** — Custom ANSI parser + virtual screen buffer (no xterm.js)
- **Custom Syntax Highlighter** — Zero-dependency regex-based highlighting for 30+ languages
- **Cloudflare Workers AI** — LLM-powered agentic terminal mode (`@cf/meta/llama-4-scout-17b-16e-instruct`)
- **tauri-plugin-stronghold** — Encrypted API token storage (argon2 + salt)
- **tauri-plugin-store** — Persistent settings (non-sensitive data)

## What's Built

### Native Terminal Emulator (`src/lib/terminal-emulator.ts`)
- **Custom ANSI/VT100 parser** — Processes raw PTY output with full escape sequence support
- **Virtual screen buffer** — 2D cell grid, each cell stores character + style (fg, bg, bold, dim, italic, underline, inverse, strikethrough)
- **Color support** — Standard 8-color, 256-color palette, and 24-bit truecolor (RGB)
- **Cursor management** — Positioning (CUP, CUU, CUD, CUF, CUB), save/restore, horizontal/vertical absolute
- **Screen operations** — Erase in display (ED), erase in line (EL), insert/delete lines and characters
- **Scroll regions** — DECSTBM scrolling regions with proper scroll up/down
- **Alternate screen buffer** — DECSET/DECRST 1049/47/1047 for TUI apps (nano, vim, less, top, etc.)
- **OSC title changes** — Terminal title updates from shell (OSC 0/2)
- **Keyboard mapping** — `keyToSequence()` translates browser keyboard events to terminal escape sequences (arrow keys, function keys, Ctrl combos, Alt combos)
- **Scrollback buffer** — 5000-line scrollback history for the primary screen

### Rust Backend (modular — `src-tauri/src/`)
- **`lib.rs`** — App orchestrator: shared state (`PtySession`, `AppState`, `TerminalOutput`), plugin setup, command registration
- **`shell.rs`** — PTY session management: `spawn_shell`, `write_to_shell`, `resize_pty`, `get_shell_cwd`
- **`files.rs`** — File system operations: `list_directory`, `read_file_contents`, `write_file_contents`
- **`ai.rs`** — Cloudflare Workers AI integration: `ai_chat` command
- **Multi-session support** — `Arc<Mutex<HashMap>>` session store powering tabs and split panes
- **Raw PTY passthrough** — No echo filtering; raw bytes sent directly to the frontend emulator

### Frontend (component-based — `src/`)
- **`+page.svelte`** — Main orchestrator: terminal rendering, tabs, split panes, input area, AI overlay
- **`EditorView.svelte`** — Built-in text editor with syntax highlighting, cursor management, save/load
- **`FileExplorer.svelte`** — Sidebar file tree with directory navigation, hidden file toggle, file type colors
- **`SettingsPage.svelte`** — Settings with sidebar navigation (AI, Theme, About tabs)
- **`theme.ts`** — Shared theme system: presets, CSS variable application, prompt highlighting, file colors
- **`types.ts`** — Shared TypeScript interfaces (`FileEntry`)
- **`terminal-emulator.ts`** — Custom ANSI parser + virtual screen buffer

#### Terminal Features
- **Two rendering modes** — Normal mode (scrollback + input bar) and TUI mode (full grid + raw keystrokes)
- **Automatic mode switching** — Detects alternate screen buffer enter/exit to toggle between modes
- **Tab support** — Multiple terminal sessions with rename (double-click), drag reorder, close
- **Dynamic tab titles** — Tabs default to current working directory; auto-updates after `cd` and via OSC title sequences
- **Split pane** — 50/50 side-by-side layout (max 2 panes), unsplit keeps both tabs alive
- **AI mode** — Dual-button pill toggle (terminal ↔ AI) in the input area
- **Natural language → command** — AI translates user intent, shows suggested command, user confirms with Enter or cancels with Escape
- **Settings page** — Sidebar-navigated settings with tabs for AI credentials, theme customization, and about
- **Secure credential storage** — API token encrypted via Stronghold; Account ID in plain store
- **Command history** — Per-tab arrow up/down to cycle through previous commands
- **Session lifecycle** — `exit` closes tab, dead sessions auto-respawn, `session-ended` events

### Terminal Theme System
- **Preset themes** — Dracula (default) and Slate, plus a fully customizable theme
- **Prompt highlighting** — Regex-based coloring of user, host, path, symbol, and command segments
- **File type colors** — 40+ extensions mapped to VS Code Dracula-style colors
- **CSS custom properties** — Theme colors applied via `--prompt-user`, `--prompt-host`, etc.
- **Persistence** — Theme selection and custom colors saved to `tauri-plugin-store`
- **Live preview** — Theme picker with color swatches, custom editor with 8 color pickers

### File Explorer (`FileExplorer.svelte`)
- **Sidebar panel** — 240px left sidebar toggled via folder icon in titlebar
- **CWD-relative root** — Opens relative to the focused tab's current working directory
- **Directory tree** — Recursive expand/collapse with lazy-loaded children
- **Breadcrumb navigation** — Current path display with up-navigation
- **Hidden files toggle** — Eye icon to show/hide dotfiles
- **File type colors** — VS Code Dracula-style coloring by extension

### Built-in Text Editor (`EditorView.svelte`)
- **Contenteditable editor** — Native cursor/selection, syntax highlighting (30+ languages)
- **Save (⌘S / Ctrl+S)** — Writes via `write_file_contents` Rust command
- **Dirty indicator** — Dot (•) in titlebar when unsaved, save status badge
- **Line numbers** — Scroll-synced gutter
- **Tab key** — Inserts 2 spaces
- **Language detection** — Auto-detects from file extension

### UI / Design (`src/styles/global.css`)
- **shadcn-style neutral palette** — `#0a0a0f` base, `#111118` surface, `#e4e4ed` accent, no purple
- **Titlebar** — Tabs + new tab (+) on the left, split pane icon + settings gear (⚙) on the right
- **Mode switch** — Dual-button pill with terminal icon and bot icon, subtle highlight on active
- **Full-width input bar** — Clean bottom bar with top divider, no bordered box — mode switch + textarea inline
- **AI line types** — Cyan for suggestions, muted italic for loading, red for errors
- **TUI grid** — Monospace grid layout for full-screen terminal apps, fixed row height
- **Split pane** — 1px divider, split header with CWD title and close button
- **Mono font stack** — JetBrains Mono → Fira Code → SF Mono → Menlo
- **macOS native feel** — Overlay titlebar with drag region, custom scrollbar, selection color
- **Status bar** — Connection indicator, mode (Terminal/AI), shell type, version

### Keyboard Shortcuts
- **⌘T** — New tab
- **⌘W** — Close tab
- **⌘L** — Clear screen / reset emulator
- **⌘S / Ctrl+S** — Save file (in editor)
- **Ctrl+C** — Interrupt running process
- **↑/↓** — Command history (normal mode)
- **Tab** — Insert 2 spaces (in editor)
- **Enter** — Execute command / confirm AI suggestion
- **Escape** — Cancel AI suggestion
- All standard keys forwarded in TUI mode (arrow keys, function keys, Ctrl combos, etc.)

## Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/learn/get-started#installing-rust) (via rustup)
- [Bun](https://bun.sh/)
- macOS (primary target)

### Development
```bash
bun install
bun run tauri dev
```

### Build
```bash
bun run tauri build
```

### AI Mode Setup
1. Click the ⚙ gear icon in the titlebar
2. Enter your Cloudflare Account ID and Workers AI API Token
3. Click Save Credentials
4. Click the bot icon in the input area to switch to AI mode

## Project Structure

```
Drover/
├── src/                           # Frontend (SvelteKit)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── EditorView.svelte  # Built-in text editor (syntax highlighting)
│   │   │   ├── FileExplorer.svelte# Sidebar file tree
│   │   │   └── SettingsPage.svelte# Settings with sidebar tabs (AI, Theme, About)
│   │   ├── terminal-emulator.ts   # Native ANSI parser + virtual screen buffer
│   │   ├── theme.ts               # Theme presets, CSS vars, prompt highlighting, file colors
│   │   └── types.ts               # Shared TypeScript interfaces
│   ├── routes/
│   │   ├── +page.svelte           # Main orchestrator (terminal, tabs, split panes, AI)
│   │   ├── +layout.svelte         # Root layout
│   │   └── +layout.ts             # SSG config
│   └── styles/
│       └── global.css             # Design tokens & all styles
├── src-tauri/                     # Rust backend
│   ├── src/
│   │   ├── lib.rs                 # App state, plugin setup, command registration
│   │   ├── shell.rs               # PTY session management (spawn, write, resize, CWD)
│   │   ├── files.rs               # File system operations (list, read, write)
│   │   ├── ai.rs                  # Cloudflare Workers AI integration
│   │   └── main.rs                # Entry point
│   ├── capabilities/
│   │   └── default.json           # Permissions (store, stronghold)
│   ├── Cargo.toml                 # Rust dependencies
│   └── tauri.conf.json            # Window & build config
└── package.json
```
