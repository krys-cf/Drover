# Drover

A modern agentic terminal built with Rust and Svelte. Native terminal emulation with AI-powered code editing — no xterm.js.

![Drover Terminal](https://img.shields.io/badge/Platform-macOS-blue) ![Rust](https://img.shields.io/badge/Rust-1.70+-orange) ![License](https://img.shields.io/badge/License-MIT-green)

## Features

### 🖥️ Native Terminal
- **Custom ANSI/VT100 emulator** — No xterm.js dependency, full escape sequence support
- **TUI app support** — vim, nano, htop, less, and other full-screen apps work perfectly
- **256-color & truecolor** — Full RGB color support
- **Scrollback history** — 5000-line buffer with smooth scrolling
- **Split panes** — Side-by-side terminal sessions
- **Tabs** — Multiple sessions with drag-to-reorder, rename, and dynamic titles

### 🤖 AI Agent Mode
- **Natural language commands** — Describe what you want, AI suggests the command
- **Agentic code editing** — AI reads, modifies, and writes code files with diff preview
- **@ mention file picker** — Type `@filename` to search and attach files to your prompt
- **Right-click to add files** — Context menu on terminal output to add files to AI
- **Image analysis** — Paste screenshots or images for AI to analyze
- **MCP server support** — Connect external tools via Model Context Protocol

### 📝 Built-in Editor
- **Syntax highlighting** — 30+ languages with zero dependencies
- **Click-to-open** — Click any file in terminal output to preview/edit
- **Markdown preview** — Live rendered markdown with syntax highlighting
- **Save with ⌘S** — Direct file system writes

### 📁 File Explorer
- **Sidebar navigation** — Browse files relative to current directory
- **Hidden file toggle** — Show/hide dotfiles
- **File type colors** — VS Code-style extension coloring
- **SSH support** — Browse remote file systems

### 🔧 Developer Tools
- **DNS lookup** — dig, trace, WHOIS
- **SSL/TLS** — Certificate inspection and decoding
- **Network** — curl, ping, traceroute, port scanning
- **All tools work over SSH** — Run diagnostics on remote servers

### 🎨 Theming
- **Preset themes** — Dracula (default), Slate, and custom
- **Prompt highlighting** — Colored user, host, path segments
- **Live preview** — See changes instantly

### ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `⌘T` | New tab |
| `⌘W` | Close tab |
| `⌘L` | Clear screen |
| `⌘S` | Save file (editor) |
| `⌘\` | Toggle split pane |
| `⌘,` | Open settings |
| `Ctrl+C` | Interrupt process |
| `↑/↓` | Command history |
| `Enter` | Execute / confirm AI |
| `Escape` | Cancel AI suggestion |

## Installation

### Prerequisites

- **macOS** (primary target, Windows/Linux coming soon)
- **[Rust](https://rustup.rs/)** — Install via rustup
- **[Bun](https://bun.sh/)** — Fast JavaScript runtime & package manager

### Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/drover.git
cd drover

# Install dependencies
bun install

# Run in development mode
bun run tauri dev
```

### Build for Production

```bash
# Create optimized release build
bun run tauri build

# Output: src-tauri/target/release/bundle/
```

The built `.app` bundle will be in `src-tauri/target/release/bundle/macos/`.

## Configuration

### AI Setup (Cloudflare Workers AI)

1. Click the **⚙ gear icon** in the titlebar
2. Navigate to the **AI** tab
3. Enter your **Cloudflare Account ID** and **Workers AI API Token**
4. Click **Save Credentials**
5. Toggle to **AI mode** using the robot icon in the input bar

Your API token is encrypted locally using Tauri Stronghold (argon2 + salt).

### MCP Servers

Connect external tools via the Model Context Protocol:

1. Go to **Settings → MCP**
2. Add server URL and optional auth token
3. Tools appear in the AI dropdown menu

## Development

### Project Structure

```
drover/
├── src/                          # Frontend (SvelteKit + Svelte 5)
│   ├── lib/
│   │   ├── components/           # Svelte components
│   │   │   ├── DiffViewer.svelte # Code diff with syntax highlighting
│   │   │   ├── EditorView.svelte # Built-in text editor
│   │   │   ├── FileExplorer.svelte
│   │   │   ├── MarkdownPreview.svelte
│   │   │   └── SettingsPage.svelte
│   │   ├── terminal-emulator.ts  # Custom ANSI parser
│   │   ├── highlight.ts          # Syntax highlighting engine
│   │   └── theme.ts              # Theme system
│   ├── routes/
│   │   └── +page.svelte          # Main app orchestrator
│   └── styles/
│       └── global.css            # All styles (no framework)
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── lib.rs                # App state & command registration
│   │   ├── shell.rs              # PTY management
│   │   ├── files.rs              # File system operations
│   │   ├── ai.rs                 # AI integration & code editing
│   │   ├── tools.rs              # Network diagnostic tools
│   │   └── mcp.rs                # MCP server client
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

### Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop Shell | Tauri 2 (Rust) |
| Frontend | SvelteKit + Svelte 5 |
| Package Manager | Bun |
| Styling | Vanilla CSS (shadcn-style tokens) |
| Terminal | Custom ANSI emulator (no xterm.js) |
| PTY | portable-pty |
| AI | Cloudflare Workers AI |
| Secrets | tauri-plugin-stronghold |
| Storage | tauri-plugin-store |

### Commands

```bash
# Development with hot reload
bun run tauri dev

# Type checking
bun run check

# Build release
bun run tauri build

# Clean build artifacts
cargo clean --manifest-path src-tauri/Cargo.toml
```

## Contributing

Contributions are welcome! Here's how to get started:

1. **Fork** the repository
2. **Create a branch** for your feature (`git checkout -b feature/amazing-feature`)
3. **Make your changes** and test thoroughly
4. **Commit** with clear messages (`git commit -m 'Add amazing feature'`)
5. **Push** to your branch (`git push origin feature/amazing-feature`)
6. **Open a Pull Request**

### Guidelines

- Follow existing code style (Rust: `cargo fmt`, TypeScript: consistent with codebase)
- Add comments for complex logic
- Test on macOS before submitting
- Keep PRs focused — one feature or fix per PR

### Areas for Contribution

- **Windows/Linux support** — PTY and window management
- **Additional themes** — New color schemes
- **Language support** — More syntax highlighting grammars
- **AI providers** — OpenAI, Anthropic, local models
- **Documentation** — Tutorials, examples, guides

## License

MIT License — see [LICENSE](LICENSE) for details.

## Acknowledgments

- [Tauri](https://tauri.app/) — Rust-powered desktop apps
- [Svelte](https://svelte.dev/) — Reactive UI framework
- [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/) — LLM inference
- [portable-pty](https://docs.rs/portable-pty/) — Cross-platform PTY
