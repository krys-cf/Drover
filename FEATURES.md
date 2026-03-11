# Drover — Feature Tracker

## Completed

### Native Terminal Emulator
- [x] **Custom ANSI/VT100 Parser** — Processes raw PTY output, no xterm.js dependency
- [x] **Virtual Screen Buffer** — 2D cell grid with per-cell character + style attributes
- [x] **ANSI Color Support** — 8-color, 256-color palette, and 24-bit truecolor (RGB)
- [x] **Text Styling** — Bold, dim, italic, underline, inverse, strikethrough
- [x] **Cursor Management** — Full cursor positioning (CUP, CUU, CUD, CUF, CUB), save/restore
- [x] **Screen Operations** — Erase in display, erase in line, insert/delete lines and characters
- [x] **Scroll Regions** — DECSTBM scrolling regions with proper scroll up/down
- [x] **Alternate Screen Buffer** — DECSET/DECRST 1049/47/1047 for TUI apps
- [x] **OSC Title Changes** — Terminal title updates from shell (OSC 0/2)
- [x] **Keyboard Mapping** — Browser keyboard events → terminal escape sequences (arrows, F-keys, Ctrl, Alt)
- [x] **Scrollback Buffer** — 5000-line scrollback history for primary screen
- [x] **Raw PTY Passthrough** — No echo filtering; raw bytes from PTY to emulator

### TUI Mode
- [x] **Full-Screen Grid Rendering** — Monospace grid layout for interactive programs
- [x] **Raw Keystroke Passthrough** — All keystrokes forwarded directly to PTY in TUI mode
- [x] **Automatic Mode Detection** — Alternate screen buffer enter/exit toggles TUI ↔ normal mode
- [x] **nano Support** — Full nano editor functionality (open, edit, save, exit)
- [x] **vim/less/man/top Support** — All TUI programs work via alternate screen buffer

### Core Terminal
- [x] **PTY Shell Backend** — Spawn real login shell via `portable-pty`, stream raw output via Tauri events
- [x] **Normal Mode Rendering** — Scrollback lines rendered as styled spans with ANSI colors
- [x] **Command History** — Per-tab arrow up/down to cycle through previous commands
- [x] **Clear Screen** — `⌘L` shortcut resets the emulator
- [x] **Ctrl+C Interrupt** — Send SIGINT to running processes
- [x] **PTY Resize** — Dynamically resize PTY and emulator when window resizes
- [x] **Exit Handling** — `exit` closes tab, `session-ended` event auto-cleans dead sessions
- [x] **Default Terminal Mode** — Starts in terminal mode, not AI mode

### Tabs & Panes
- [x] **Tab Support** — Multiple terminal sessions with independent emulator instances
- [x] **Tab Rename** — Double-click tab to rename, Enter/Escape to confirm/cancel
- [x] **Tab Drag Reorder** — Pointer-event based drag to reorder tabs
- [x] **Tab Close** — × button, ⌘W shortcut, auto-respawn on last tab close
- [x] **Dynamic Tab Titles** — CWD-based titles, auto-updates after commands and via OSC sequences
- [x] **CWD Lookup** — Rust `get_shell_cwd` command reads shell PID's CWD via `lsof` (macOS)
- [x] **Split Pane** — 50/50 side-by-side layout, independent shell session, max 2 panes
- [x] **Unsplit Keeps Tabs** — Unsplitting moves the split tab into the tab bar instead of killing it
- [x] **Focused Pane Management** — Click/focus correctly tracks which pane has input focus
- [x] **Split Pane Controls** — Titlebar toggle icon, header with CWD title + close (×) button

### Agentic Terminal (AI Mode)
- [x] **Cloudflare Workers AI Integration** — `ai_chat` command in `ai.rs` calls `@cf/meta/llama-4-scout-17b-16e-instruct` via REST
- [x] **Encrypted API Token** — Stored in `tauri-plugin-stronghold` (argon2 + salt), never on plain disk
- [x] **Account ID Storage** — Non-sensitive data in `tauri-plugin-store` (settings.json)
- [x] **Mode Switch** — Dual-button pill (terminal icon ↔ bot icon) in input area, shadcn-style
- [x] **Natural Language → Command** — AI translates user intent to shell commands
- [x] **Command Confirmation** — Suggested command shown in cyan; Enter to run, Escape to cancel
- [x] **AI Overlay** — AI prompts/suggestions rendered as overlay, separate from terminal buffer
- [x] **Settings Page** — Sidebar-navigated settings (`SettingsPage.svelte`) with AI, Theme, and About tabs
- [x] **Show/Hide Token** — Toggle to reveal or mask the API token in settings
- [x] **Revoke Credentials** — Clear both Account ID and API token from storage
- [x] **AI Line Types** — Distinct styling: 🤖 prompt, 💡 suggestion (cyan), ⏳ loading (italic), ⚠ error (red)

### File Explorer (`FileExplorer.svelte`)
- [x] **Sidebar Panel** — 240px left sidebar toggled via folder icon in titlebar
- [x] **CWD-Relative Root** — Opens relative to the focused tab's current working directory
- [x] **Directory Tree** — Recursive tree with expand/collapse, lazy-loaded children from backend
- [x] **Breadcrumb Navigation** — Current path display with up-navigation button
- [x] **Hidden Files Toggle** — Eye icon to show/hide dotfiles
- [x] **File Type Colors** — VS Code Dracula-style coloring by file extension (JS yellow, TS blue, Rust orange, etc.)
- [x] **Backup File Filtering** — `.save` files hidden from explorer
- [x] **Rust Backend** — `list_directory` in `files.rs` returns sorted entries (directories first, alphabetical)

### Built-in Text Editor (`EditorView.svelte`)
- [x] **Contenteditable Editor** — Native cursor/selection via `contenteditable` div, no textarea overlay
- [x] **Syntax Highlighting** — Dracula-inspired theme, 30+ language support, zero dependencies
- [x] **Language Detection** — Auto-detects language from file extension via `detectLang()` in `theme.ts`
- [x] **Save (⌘S / Ctrl+S)** — Writes file contents via `write_file_contents` Rust command
- [x] **Dirty Indicator** — Dot (•) in titlebar when unsaved changes
- [x] **Save Status Badge** — "Saving...", "Saved" (green), "Save Failed" (red) in titlebar
- [x] **Save Icon Button** — Floppy disk icon in editor titlebar, disabled when clean
- [x] **Tab Key** — Inserts 2 spaces
- [x] **Line Numbers** — Gutter with line numbers, scroll-synced with editor
- [x] **Cursor Restore** — Preserves cursor position across re-highlighting
- [x] **Rust Backend** — `read_file_contents` (10MB limit) and `write_file_contents` in `files.rs`

### Terminal Theme System (`theme.ts`)
- [x] **Preset Themes** — Dracula (default) and Slate, plus a fully customizable theme
- [x] **Custom Theme Builder** — 8 color pickers in 2-column grid for building a personal theme
- [x] **Prompt Highlighting** — Regex-based coloring of user, host, path, symbol, and command segments
- [x] **File Type Colors** — 40+ extensions mapped to VS Code Dracula-style colors via `FILE_COLOR_MAP`
- [x] **CSS Custom Properties** — Theme colors applied via `--prompt-user`, `--prompt-host`, etc.
- [x] **Theme Persistence** — Theme selection and custom colors saved to `tauri-plugin-store`
- [x] **Live Preview** — Theme picker with color swatches in settings

### Terminal Output Coloring
- [x] **iTerm-style Output** — Colorized terminal output beyond raw ANSI codes
- [x] **File Type Coloring** — Directories blue, executables green, symlinks cyan in ls output
- [x] **Prompt Highlighting** — Username, host, path components colored per active theme

### Architecture
- [x] **Modular Rust Backend** — `lib.rs` (orchestrator), `shell.rs`, `files.rs`, `ai.rs`
- [x] **Component-Based Frontend** — `EditorView`, `FileExplorer`, `SettingsPage` extracted from monolithic page
- [x] **Shared Theme Module** — `theme.ts` centralizes theme types, presets, CSS vars, file colors, language detection
- [x] **Shared Types** — `types.ts` for cross-component interfaces (`FileEntry`)
- [x] **Settings Sidebar** — 180px sidebar with icon+label nav items for AI, Theme, About tabs

### UI / Design
- [x] **shadcn-style Neutral Palette** — `#0a0a0f` base, `#111118` surface, `#e4e4ed` accent, no purple
- [x] **Full-Width Input Bar** — Clean bottom bar with top divider, no bordered box — mode switch + textarea inline
- [x] **TUI Grid CSS** — Monospace grid layout for full-screen terminal apps, fixed row height
- [x] **macOS Native Titlebar** — Overlay titlebar with drag region
- [x] **Titlebar Layout** — Tabs + new tab on left, split icon + settings gear on right
- [x] **Window Dragging** — `data-tauri-drag-region` + permission for Tauri 2
- [x] **Text Selection & Copy** — `user-select: text` on output, smart focus management
- [x] **Status Bar** — Connection indicator, mode (Terminal/AI/Editor), shell type or language, version
- [x] **Mono Font Stack** — JetBrains Mono → Fira Code → SF Mono → Menlo

---

## Planned

### UI Enhancements
- [ ] **Clickable URLs** — Detect and linkify URLs in output
- [ ] **Search in Output** — `⌘F` to search through terminal history
- [ ] **Auto-complete** — Tab completion forwarded from shell
- [ ] **Resizable Split Pane** — Drag divider to resize panes
- [ ] **Cursor Rendering** — Visible blinking cursor in TUI mode
- [ ] **Editor: Undo/Redo** — Full undo/redo support in the built-in editor
- [ ] **Light Mode Theme** — Light color scheme option

### Backend Improvements
- [ ] **Shell Detection** — Auto-detect and display current shell (zsh, bash, fish)
- [ ] **Session Persistence** — Restore sessions after app restart
- [ ] **Performance** — Virtualized line rendering for massive scrollback

### AI Enhancements
- [ ] **Streaming AI Responses** — Stream tokens as they arrive instead of waiting for full response
- [ ] **Context-aware Prompts** — Include CWD, OS, recent commands in AI context
- [ ] **Model Selection** — Choose between different Cloudflare Workers AI models in settings
- [ ] **Command History for AI** — AI can reference previous commands and output

### Platform
- [ ] **Linux Support** — Replace `lsof` CWD lookup with `/proc/PID/cwd`
- [ ] **Windows Support** — Test with PowerShell / CMD / WSL
- [ ] **Auto-update** — Tauri updater integration
