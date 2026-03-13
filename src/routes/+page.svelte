<script lang="ts">
  import { onMount, tick, flushSync } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { load } from '@tauri-apps/plugin-store';
  import { Client, Stronghold } from '@tauri-apps/plugin-stronghold';
  import { appDataDir, homeDir as getHomeDir } from '@tauri-apps/api/path';
  import { AgentClient } from 'agents/client';
  import { TerminalEmulator, keyToSequence, encodeMouseEvent, type CellStyle } from '$lib/terminal-emulator';
  import { TerminalCanvas } from '$lib/terminal-canvas';
  import { THEMES, applyThemeCssVars, highlightPrompt as themeHighlightPrompt, isBarePrompt, highlightOutput, type TerminalTheme } from '$lib/theme';
  import type { FileEntry } from '$lib/types';
  import SettingsPage from '$lib/components/SettingsPage.svelte';
  import EditorView from '$lib/components/EditorView.svelte';
  import MarkdownPreview from '$lib/components/MarkdownPreview.svelte';
  import { renderMarkdown } from '$lib/markdown';
  import FileExplorer from '$lib/components/FileExplorer.svelte';
  import DigApp from '$lib/components/DigApp.svelte';
  import CurlApp from '$lib/components/CurlApp.svelte';
  import OpensslApp from '$lib/components/OpensslApp.svelte';
  import WhoisApp from '$lib/components/WhoisApp.svelte';
  import PingApp from '$lib/components/PingApp.svelte';
  import TracerouteApp from '$lib/components/TracerouteApp.svelte';
  import PortScanApp from '$lib/components/PortScanApp.svelte';
  import McpExplorer from '$lib/components/McpExplorer.svelte';
  import PaneLayout from '$lib/components/PaneLayout.svelte';
  import DiffViewer from '$lib/components/DiffViewer.svelte';
  import { createInitialLayout, splitPane, closePane, getAllLeaves, findPane, swapPanes, deepCloneNode, type PaneLayout as PaneLayoutType, type PaneLeaf, type PaneNode, type PaneContainer } from '$lib/pane-layout';

  let charWidth = 7.22;
  let charHeight = 16;

  type DesktopPlatform = 'macos' | 'windows' | 'linux' | 'unknown';
  let desktopPlatform = $state<DesktopPlatform>('unknown');

  function detectDesktopPlatform(): DesktopPlatform {
    if (typeof navigator === 'undefined') return 'unknown';

    const rawPlatform = (
      (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ||
      navigator.platform ||
      navigator.userAgent ||
      ''
    ).toLowerCase();

    if (rawPlatform.includes('mac')) return 'macos';
    if (rawPlatform.includes('win')) return 'windows';
    if (rawPlatform.includes('linux')) return 'linux';
    return 'unknown';
  }

  function isPrimaryModifier(e: KeyboardEvent): boolean {
    return desktopPlatform === 'macos' ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  }

  function shortcutLabel(key: string): string {
    return desktopPlatform === 'macos' ? `Cmd+${key}` : `Ctrl+${key}`;
  }

  function splitShortcutLabel(direction: 'top' | 'bottom' | 'left' | 'right'): string {
    const arrow = {
      top: 'Up',
      bottom: 'Down',
      left: 'Left',
      right: 'Right',
    }[direction];

    return desktopPlatform === 'macos' ? `Cmd+Shift+${arrow}` : `Ctrl+Shift+${arrow}`;
  }

  function getLocalShellLabel(): string {
    return desktopPlatform === 'windows' ? 'PowerShell' : 'zsh';
  }

  function measureCharMetrics() {
    const probe = document.createElement('span');
    probe.style.cssText = `position:absolute;visibility:hidden;white-space:pre;font-family:var(--font-mono);font-size:var(--font-size-md);line-height:1.2;`;
    probe.textContent = 'W';
    document.body.appendChild(probe);
    charWidth = probe.getBoundingClientRect().width;
    charHeight = probe.getBoundingClientRect().height;
    document.body.removeChild(probe);
    document.documentElement.style.setProperty('--char-height', `${charHeight}px`);
  }

  function handleInputPaste(e: ClipboardEvent, tab: Tab) {
    if (!tab.connected || !tab.commandRunning) return;
    const text = e.clipboardData?.getData('text/plain') ?? '';
    if (!text) return;
    e.preventDefault();
    invoke('write_to_shell', { sessionId: tab.sessionId, data: text });
  }

  class Tab {
    id: string;
    sessionId: string;
    title = $state('');
    customTitle = $state(false);
    emulator: TerminalEmulator;
    renderVersion = $state(0);
    connected = $state(true);
    commandHistory: string[] = $state([]);
    historyIndex = $state(-1);
    inputValue = $state('');
    sshTarget = $state('');
    remoteOs = $state('');
    remoteShell = $state('');
    cwd = $state('~');
    cwdFromOsc = $state(false);
    commandRunning = $state(false);
    awaitingInput = $state(false);

    isTui = $derived.by(() => {
      void this.renderVersion;
      return this.emulator?.useAltBuffer ?? false;
    });

    renderedLines = $derived.by(() => {
      void this.renderVersion;
      return this.emulator?.getScrollbackLines() ?? [];
    });

    gridLines = $derived.by(() => {
      void this.renderVersion;
      return this.emulator?.getLines() ?? [];
    });

    constructor(id: string, sessionId: string, emulator: TerminalEmulator) {
      this.id = id;
      this.sessionId = sessionId;
      this.emulator = emulator;
    }
  }

  let tabs: Tab[] = $state([]);
  let activeTabId = $state('');
  let homeDir = $state('');

  let paneLayout = $state<PaneLayoutType | null>(null);
  let paneInputEls: Record<string, HTMLTextAreaElement> = {};
  let paneTuiEls: Record<string, HTMLDivElement> = {};
  let paneOutputEls: Record<string, HTMLDivElement> = {};
  let tuiCanvases: Record<string, TerminalCanvas> = {};
  let terminalContainerEl: HTMLDivElement = undefined as unknown as HTMLDivElement;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;
  let unlistenOutput: UnlistenFn | null = null;
  let unlistenEnded: UnlistenFn | null = null;
  let tabCounter = 0;
  let creatingTab = false;
  let dirtyTabIds = new Set<string>();
  let rafPending = false;
  let editingTabId = $state('');
  let editingTitle = $state('');
  let dragTabId = $state('');
  let dragOverTabId = $state('');
  let pointerStartX = 0;
  let isDragging = $state(false);

  // Zoom state
  let zoomLevel = $state(100);
  const ZOOM_STEP = 10;
  const ZOOM_MIN = 50;
  const ZOOM_MAX = 200;

  function applyZoom() {
    (document.body.style as any).zoom = `${zoomLevel}%`;
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && (e.key === '=' || e.key === '+')) {
      e.preventDefault();
      zoomLevel = Math.min(ZOOM_MAX, zoomLevel + ZOOM_STEP);
      applyZoom();
    } else if (mod && e.key === '-') {
      e.preventDefault();
      zoomLevel = Math.max(ZOOM_MIN, zoomLevel - ZOOM_STEP);
      applyZoom();
    } else if (mod && e.key === '0') {
      e.preventDefault();
      zoomLevel = 100;
      applyZoom();
    }
  }

  // File explorer state
  let fileExplorerOpen = $state(false);
  let fileExplorerRoot = $state('~');
  let showHiddenFiles = $state(false);

  function toggleFileExplorer() {
    fileExplorerOpen = !fileExplorerOpen;
  }

  async function getFileExplorerCwd(): Promise<string | null> {
    const tab = focusedTab();
    if (tab && tab.connected) {
      if (tab.sshTarget) {
        try {
          const info: { is_ssh: boolean; target: string; remote_cwd: string } = await invoke('detect_ssh', { sessionId: tab.sessionId });
          return info.remote_cwd || '/';
        } catch { return '/'; }
      }
      // Prefer the OSC 7-tracked cwd (always current) over the backend
      // query which returns stale data on Windows.
      if (tab.cwdFromOsc) return tab.cwd || null;
      try {
        const cwd: string = await invoke('get_shell_cwd', { sessionId: tab.sessionId });
        return cwd || null;
      } catch { return null; }
    }
    return null;
  }

  async function handleExplorerNavigate(path: string) {
    const tab = focusedTab();
    if (!tab || !tab.connected) return;
    try {
      await invoke('write_to_shell', { sessionId: tab.sessionId, data: `cd '${path}'\r` });
      setTimeout(() => updateTabState(tab), 500);
    } catch (e) {
      console.error('Failed to cd terminal:', e);
    }
  }

  // Agentic mode state
  let agenticMode = $state(false);
  let aiLoading = $state(false);
  let pendingCommand = $state('');
  let aiExecMode = $state<'auto' | 'manual' | null>(null);
  let aiExecPromptShown = $state(false);
  let currentView = $state<'terminal' | 'settings'>('terminal');

  function transition(fn: () => void) {
    if (document.startViewTransition) {
      document.startViewTransition(() => {
        flushSync(fn);
      });
    } else {
      fn();
    }
  }
  
  // AI attachments and model selection
  interface AiAttachment {
    name: string;
    path: string;
    content: string;
    mimeType: string;
  }
  interface AiSessionSummary {
    id: string;
    title?: string | null;
    model: string;
    lastUserMessage?: string | null;
    lastAssistantMessage?: string | null;
    messageCount: number;
    created_at: string;
    updated_at: string;
  }
  interface AiChatTurn {
    session_id: string;
    response: string;
  }
  interface LiveIdeState {
    sessionId: string;
    organizationId: string;
    model: string;
    phase: 'idle' | 'thinking' | 'approval' | 'executing' | 'answered' | 'needs_input' | 'error';
    prompt: string;
    raw: string;
    response: string;
    error: string;
    commands: AiCmdEntry[];
  }
  interface LiveAgentConnectResponse {
    session: AiSessionSummary;
    agent: {
      agent: string;
      name: string;
      host: string;
      protocol: 'ws' | 'wss';
      query: Record<string, string>;
      expiresAt: string;
    };
  }
  interface AiSessionDetailState {
    sessionId: string;
    organizationId: string;
    model: string;
    phase: 'idle' | 'thinking' | 'approval' | 'executing' | 'answered' | 'needs_input' | 'error';
    prompt: string;
    raw: string;
    response: string;
    error: string;
    commands: AiCmdEntry[];
  }
  interface AiSessionDetail extends AiSessionSummary {
    state: AiSessionDetailState | null;
  }
  let aiAttachments = $state<AiAttachment[]>([]);
  let aiModelDropdownOpen = $state(false);
  let selectedAiModel = $state('@cf/meta/llama-4-scout-17b-16e-instruct');
  let aiSessions = $state<AiSessionSummary[]>([]);
  let aiSessionsSidebarOpen = $state(false);
  let aiSessionSearch = $state('');
  let aiSessionsLoading = $state(false);
  let aiSessionsError = $state('');
  let aiSessionActionError = $state('');
  let currentAiSessionId = $state('');
  let liveAgentActiveBlockId = $state(0);
  let liveAgentConnecting = $state(false);
  let liveAgentSessionName = $state('');
  let liveAgentClient: AgentClient<LiveIdeState> | null = null;
  
  // Code editing state
  interface DiffLine {
    tag: string;
    content: string;
    old_line: number | null;
    new_line: number | null;
  }
  interface CodeEditResult {
    file_name: string;
    file_path: string | null;
    original: string;
    modified: string;
    diff_lines: DiffLine[];
    summary: string;
  }
  let pendingCodeEdit = $state<CodeEditResult | null>(null);
  let applyingCodeEdit = $state(false);
  
  type AiModelEntry = { id: string; name: string; description: string; provider: 'cloudflare' | 'gemini' };
  const AI_MODELS: AiModelEntry[] = [
    // Cloudflare (via Kuratchi)
    { id: '@cf/meta/llama-4-scout-17b-16e-instruct', name: 'Llama 4 Scout (17B)', description: 'Fast, efficient model', provider: 'cloudflare' },
    { id: '@cf/meta/llama-3.1-8b-instruct', name: 'Llama 3.1 (8B)', description: 'Balanced performance', provider: 'cloudflare' },
    { id: '@cf/meta/llama-3.1-70b-instruct', name: 'Llama 3.1 (70B)', description: 'High quality responses', provider: 'cloudflare' },
    { id: '@cf/meta/llama-3.2-3b-instruct', name: 'Llama 3.2 (3B)', description: 'Lightweight and fast', provider: 'cloudflare' },
    { id: '@cf/meta/llama-3.2-11b-vision-instruct', name: 'Llama 3.2 Vision (11B)', description: 'Vision + text understanding', provider: 'cloudflare' },
    { id: '@cf/meta/llama-3.3-70b-instruct-fp8-fast', name: 'Llama 3.3 (70B) Fast', description: 'Optimized for speed', provider: 'cloudflare' },
    { id: '@cf/qwen/qwen2.5-14b-instruct', name: 'Qwen 2.5 (14B)', description: 'Strong reasoning', provider: 'cloudflare' },
    { id: '@cf/qwen/qwen2.5-72b-instruct', name: 'Qwen 2.5 (72B)', description: 'Advanced reasoning', provider: 'cloudflare' },
    { id: '@cf/deepseek-ai/deepseek-r1-distill-llama-70b', name: 'DeepSeek R1 (70B)', description: 'Reasoning specialist', provider: 'cloudflare' },
    // Google Gemini (direct API)
    { id: 'gemini-2.5-flash', name: 'Gemini 2.5 Flash', description: 'Fast + thinking', provider: 'gemini' },
    { id: 'gemini-2.5-pro', name: 'Gemini 2.5 Pro', description: 'Most capable', provider: 'gemini' },
    { id: 'gemini-2.0-flash', name: 'Gemini 2.0 Flash', description: 'Speed optimized', provider: 'gemini' },
    { id: 'gemini-2.0-flash-lite', name: 'Gemini 2.0 Flash Lite', description: 'Lightweight', provider: 'gemini' },
  ];

  function getSelectedModelProvider(): 'cloudflare' | 'gemini' {
    return AI_MODELS.find(m => m.id === selectedAiModel)?.provider || 'cloudflare';
  }

  // Keep backward compat alias
  const CLOUDFLARE_AI_MODELS = AI_MODELS.filter(m => m.provider === 'cloudflare');

  const ERROR_PATTERNS = [
    /is not recognized as the name of a cmdlet/i,
    /is not recognized as an internal or external command/i,
    /command not found/i,
    /CommandNotFoundException/,
    /No such file or directory/i,
    /Access is denied/i,
    /PermissionError/i,
    /permission denied/i,
    /cannot find the path/i,
    /The term '.+' is not recognized/i,
    /: not found$/m,
    /Unable to execute/i,
    /failed with exit code/i,
    /fatal error/i,
    /^ERROR:/m,
  ];

  function looksLikeError(output: string): boolean {
    if (!output.trim()) return false;
    return ERROR_PATTERNS.some(p => p.test(output));
  }

  const PASSTHROUGH_CMDS = new Set([
    'ls','ll','la','cd','pwd','cat','head','tail','less','more','echo','date','whoami',
    'hostname','uname','uptime','df','du','free','top','htop','ps','kill','clear','reset',
    'mkdir','rmdir','touch','cp','mv','rm','ln','chmod','chown','chgrp',
    'find','grep','awk','sed','sort','uniq','wc','cut','tr','xargs',
    'tar','gzip','gunzip','zip','unzip','curl','wget','ping','ssh','scp',
    'git','docker','npm','yarn','bun','pip','python','python3','node',
    'which','whereis','type','file','stat','id','env','export','source','history',
    'man','help','alias','vim','vi','nano','code',
  ]);

  function isBasicShellCommand(input: string): boolean {
    const trimmed = input.trim();
    if (!trimmed) return false;
    // If it starts with a known command binary, treat as passthrough
    const firstWord = trimmed.split(/[\s;|&]/)[0].replace(/^sudo\s+/, '');
    const base = firstWord.split('/').pop() || firstWord;
    return PASSTHROUGH_CMDS.has(base);
  }
  let rightPane = $state<'editor' | 'preview' | 'markdown' | 'diff' | null>(null);
  let activeTool = $state<string | null>(null);
  let toolsDropdownOpen = $state(false);

  function toggleTool(toolId: string) {
    if (activeTool === toolId) {
      activeTool = null;
    } else {
      activeTool = toolId;
    }
    toolsDropdownOpen = false;
  }

  function closeTool() {
    activeTool = null;
  }

  // Editor state (bound to EditorView component)
  let editorFilePath = $state('');
  let editorFileName = $state('');
  let editorContent = $state('');
  let editorDirty = $state(false);
  let editorSaveStatus = $state<'' | 'saving' | 'saved' | 'error'>('');
  let editorLang = $state('text');
  let editorLoading = $state(false);
  let editorError = $state('');
  let editorViewRef: EditorView | null = null;
  let editorSshTarget = $state('');

  // Markdown preview state
  let mdPreviewFilePath = $state('');
  let mdPreviewFileName = $state('');
  let mdPreviewLoading = $state(false);
  let mdPreviewError = $state('');
  let mdPreviewSshTarget = $state('');
  let mdPreviewRef: MarkdownPreview | null = null;

  // Global command history (persisted, deduplicated, max 500)
  let globalHistory: string[] = $state([]);
  let autocompleteSuggestion = $state('');

  // SSH Sessions manager
  interface SshSession { id: string; nickname: string; command: string; }
  let sshSessions: SshSession[] = $state([]);
  let sshDropdownOpen = $state(false);

  // Saved Commands manager
  interface SavedCommand { id: string; name: string; command: string; category: 'ssh' | 'quick'; }
  let savedCommands: SavedCommand[] = $state([]);
  let slashMenuOpen = $state(false);
  let slashMenuIndex = $state(0);
  let slashFilter = $state('');

  // @ mention file picker state
  interface FileSuggestion { name: string; path: string; isDir: boolean; }
  let atMenuOpen = $state(false);
  let atMenuIndex = $state(0);
  let atFilter = $state('');
  let atFileSuggestions: FileSuggestion[] = $state([]);
  let atMenuLoading = $state(false);

  // MCP Servers manager
  interface McpServerEntry { name: string; server_url: string; auth_token?: string; auth_command?: string; }
  let mcpServers: McpServerEntry[] = $state([]);


  // Theme state
  let activeThemeId = $state('dracula');
  let customTheme = $state<TerminalTheme>({ ...THEMES.custom });

  function getTheme(): TerminalTheme {
    if (activeThemeId === 'custom') return customTheme;
    return THEMES[activeThemeId] || THEMES.dracula;
  }

  function highlightPrompt(lineText: string) {
    return themeHighlightPrompt(lineText, getTheme());
  }

  const INTERACTIVE_PROMPT_RE = /\b(password|passphrase|otp|verification code|auth(?:entication)? code|token|2fa|two-factor|username|login)\s*[:>]\s*$|\b(do you want to continue|continue\?|proceed\?|yes to all|no to all|default is|selection:|\[y\] yes|\[a\] yes to all|\[n\] no|\[l\] no to all|\[s\] suspend|\[\?\] help|\[y\/n\]|\[y\/N\]|\(y\/n\)|press enter)\b/i;

  function isLikelyInteractivePrompt(lineText: string): boolean {
    const t = lineText.trim();
    if (!t) return false;
    // Don't treat raw HTML/XML tags or attributes as prompts
    if (t.endsWith('>') || t.endsWith('="') || t.endsWith("='")) return false;
    
    if (INTERACTIVE_PROMPT_RE.test(t)) return true;
    
    // Check for standard prompt endings like ':', '?', or '> '
    if (/[:?]$/.test(t) && t.length <= 80 && !highlightPrompt(t)) return true;
    
    return false;
  }

  function detectAwaitingInput(lines: { text: string; style: CellStyle }[][], cursorRow: number): boolean {
    if (cursorRow < 0 || cursorRow >= lines.length) return false;
    const minRow = Math.max(0, cursorRow - 2);

    for (let r = cursorRow; r >= minRow; r--) {
      const row = lines[r];
      if (!row) continue;
      const text = row.map(run => run.text).join('').trimEnd();
      if (!text.trim()) continue;
      const hasAnsi = row.some(run => run.style.fg || run.style.bg || run.style.bold || run.style.italic || run.style.underline);
      if (!hasAnsi && highlightPrompt(text)) return false;
      return isLikelyInteractivePrompt(text);
    }

    return false;
  }

  function extractInteractivePrompt(output: string): string | null {
    const lines = output
      .split('\n')
      .map(line => line.trimEnd())
      .filter(line => line.trim().length > 0);

    let matchIndex = -1;
    for (let i = lines.length - 1; i >= 0; i--) {
      if (isLikelyInteractivePrompt(lines[i])) {
        matchIndex = i;
        break;
      }
    }

    if (matchIndex === -1) return null;

    const start = Math.max(0, matchIndex - 5);
    return lines.slice(start, matchIndex + 1).join('\n').trim();
  }

  function buildInteractivePromptResponse(promptText: string): string {
    const escapedPrompt = promptText.replace(/```/g, '``` ');
    return [
      'This command is waiting for input before it can continue.',
      '',
      'Respond in the terminal input below to continue, or press `Ctrl+C` to cancel it.',
      '',
      'Prompt:',
      '```text',
      escapedPrompt,
      '```',
    ].join('\n');
  }

  let aiAccountId = $state('');
  let aiApiToken = $state('');
  let geminiApiKey = $state('');

  async function getStrongholdStore() {
    const vaultPath = `${await appDataDir()}/vault.hold`;
    const stronghold = await Stronghold.load(vaultPath, 'drover-vault-pw');
    let client: Client;
    try {
      client = await stronghold.loadClient('drover');
    } catch {
      client = await stronghold.createClient('drover');
    }
    return { stronghold, store: client.getStore() };
  }

  async function strongholdInsert(key: string, value: string) {
    const { stronghold, store } = await getStrongholdStore();
    const data = Array.from(new TextEncoder().encode(value));
    await store.insert(key, data);
    await stronghold.save();
  }

  async function strongholdGet(key: string): Promise<string> {
    try {
      const { store } = await getStrongholdStore();
      const data = await store.get(key);
      if (!data || data.length === 0) return '';
      return new TextDecoder().decode(new Uint8Array(data));
    } catch {
      return '';
    }
  }

  async function strongholdRemove(key: string) {
    try {
      const { stronghold, store } = await getStrongholdStore();
      await store.remove(key);
      await stronghold.save();
    } catch { /* key may not exist */ }
  }

  async function saveThemeToStore() {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      await store.set('theme_id', activeThemeId);
      if (activeThemeId === 'custom') {
        await store.set('custom_theme', JSON.parse(JSON.stringify(customTheme)));
      }
      await store.save();
    } catch (e) {
      console.error('Failed to save theme:', e);
    }
  }

  async function loadSettings() {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      aiAccountId = (await store.get<string>('kuratchi_base_url'))
        || (await store.get<string>('ai_account_id'))
        || '';
      const savedThemeId = await store.get<string>('theme_id');
      if (savedThemeId && THEMES[savedThemeId]) {
        activeThemeId = savedThemeId;
      }
      if (savedThemeId === 'custom') {
        const saved = await store.get<TerminalTheme>('custom_theme');
        if (saved) customTheme = { ...THEMES.custom, ...saved };
      }
      applyThemeCssVars(getTheme());
      // Load global command history
      const savedHistory = await store.get<string[]>('global_history');
      if (savedHistory) globalHistory = savedHistory;
      // Load SSH sessions
      const savedSsh = await store.get<SshSession[]>('ssh_sessions');
      if (savedSsh) sshSessions = savedSsh;
      // Load saved commands
      const savedCmds = await store.get<SavedCommand[]>('saved_commands');
      if (savedCmds) savedCommands = savedCmds;
      // Load MCP servers from drover.json
      try {
        const servers: McpServerEntry[] = await invoke('mcp_list_servers');
        mcpServers = servers;
      } catch { /* ignore */ }
    } catch (e) {
      console.error('Failed to load store settings:', e);
    }
    try {
      aiApiToken = await strongholdGet('kuratchi_api_token');
      if (!aiApiToken) aiApiToken = await strongholdGet('ai_api_token');
    } catch (e) {
      console.error('Failed to load stronghold token:', e);
    }
    try {
      geminiApiKey = await strongholdGet('gemini_api_key');
    } catch (e) {
      console.error('Failed to load Gemini API key:', e);
    }
    if (aiAccountId && aiApiToken) {
      await refreshAiSessions();
    }
  }

  let saveHistoryTimer: ReturnType<typeof setTimeout> | null = null;
  function saveGlobalHistory() {
    if (saveHistoryTimer) clearTimeout(saveHistoryTimer);
    saveHistoryTimer = setTimeout(async () => {
      try {
        const store = await load('settings.json', { autoSave: true, defaults: {} });
        await store.set('global_history', globalHistory.slice(0, 500));
        await store.save();
      } catch { /* ignore */ }
    }, 2000);
  }

  async function saveSshSessions() {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      await store.set('ssh_sessions', JSON.parse(JSON.stringify(sshSessions)));
      await store.save();
    } catch { /* ignore */ }
  }


  function addSshSession(nickname: string, command: string) {
    const id = `ssh-${Date.now()}`;
    sshSessions = [...sshSessions, { id, nickname, command }];
    saveSshSessions();
  }

  function removeSshSession(id: string) {
    sshSessions = sshSessions.filter(s => s.id !== id);
    saveSshSessions();
  }

  async function saveSavedCommands() {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      await store.set('saved_commands', JSON.parse(JSON.stringify(savedCommands)));
      await store.save();
    } catch { /* ignore */ }
  }

  function addSavedCommand(name: string, command: string, category: 'ssh' | 'quick') {
    const id = `cmd-${Date.now()}`;
    const normalizedName = name.toLowerCase().replace(/[^a-z0-9_-]/g, '-');
    savedCommands = [...savedCommands, { id, name: normalizedName, command, category }];
    saveSavedCommands();
  }

  function removeSavedCommand(id: string) {
    savedCommands = savedCommands.filter(c => c.id !== id);
    saveSavedCommands();
  }

  function getFilteredSlashCommands(): SavedCommand[] {
    if (!slashFilter) return savedCommands;
    const lower = slashFilter.toLowerCase();
    return savedCommands.filter(c => c.name.toLowerCase().includes(lower) || c.command.toLowerCase().includes(lower));
  }

  function selectSlashCommand(cmd: SavedCommand) {
    const tab = focusedTab();
    if (!tab) return;
    tab.inputValue = cmd.command;
    slashMenuOpen = false;
    slashFilter = '';
    slashMenuIndex = 0;
    tick().then(() => focusInput());
  }

  function closeSlashMenu() {
    slashMenuOpen = false;
    slashFilter = '';
    slashMenuIndex = 0;
  }

  function relativeFilePath(fullPath: string): string {
    const tab = focusedTab();
    if (!tab?.cwd) return fullPath.replace(/\\/g, '/');
    const cwd = tab.cwd.replace(/\\/g, '/').replace(/\/$/, '');
    const norm = fullPath.replace(/\\/g, '/');
    if (norm.startsWith(cwd + '/')) return norm.slice(cwd.length + 1);
    return norm;
  }

  function closeAtMenu() {
    atMenuOpen = false;
    atFilter = '';
    atMenuIndex = 0;
    atFileSuggestions = [];
  }

  async function searchFilesForAtMenu(query: string) {
    const tab = focusedTab();
    if (!tab || !query) {
      atFileSuggestions = [];
      return;
    }
    
    atMenuLoading = true;
    try {
      // Use fd/find to search for files matching the query in cwd
      const cwd = tab.cwd || '~';
      const results = await invoke<{ name: string; is_dir: boolean; path: string }[]>('search_files', {
        directory: cwd,
        pattern: query,
        maxResults: 10,
      });
      atFileSuggestions = results.map(r => ({ name: r.name, path: r.path, isDir: r.is_dir }));
    } catch (e) {
      console.error('File search failed:', e);
      atFileSuggestions = [];
    }
    atMenuLoading = false;
  }

  async function selectAtFile(file: FileSuggestion) {
    if (file.isDir) {
      // If directory, update filter to show contents
      atFilter = file.name + '/';
      searchFilesForAtMenu(atFilter);
      return;
    }
    
    const tab = focusedTab();
    if (!tab) return;
    
    // Read file content and add to attachments
    try {
      let content: string;
      if (tab.sshTarget) {
        content = await invoke<string>('read_remote_file', { sshTarget: tab.sshTarget, path: file.path });
      } else {
        content = await invoke<string>('read_file_contents', { path: file.path });
      }
      
      // Determine MIME type
      const ext = file.name.split('.').pop()?.toLowerCase() || '';
      const mimeMap: Record<string, string> = {
        ts: 'text/typescript', tsx: 'text/typescript',
        js: 'text/javascript', jsx: 'text/javascript',
        rs: 'text/rust', py: 'text/python', go: 'text/go',
        svelte: 'text/svelte', vue: 'text/vue',
        html: 'text/html', css: 'text/css', scss: 'text/scss',
        json: 'application/json', yaml: 'text/yaml', yml: 'text/yaml',
        toml: 'text/toml', md: 'text/markdown', mdx: 'text/markdown',
        sh: 'text/x-shellscript', bash: 'text/x-shellscript',
        sql: 'text/sql', xml: 'text/xml', txt: 'text/plain',
      };
      const mimeType = mimeMap[ext] || 'text/plain';
      
      // Add to attachments (avoid duplicates)
      const existing = aiAttachments.find(a => a.path === file.path);
      if (!existing) {
        aiAttachments = [...aiAttachments, {
          name: file.name,
          path: file.path,
          content,
          mimeType,
        }];
      }
      
      // Remove the @query from input (e.g., "@inde" becomes "")
      const val = tab.inputValue;
      const atIndex = val.lastIndexOf('@');
      if (atIndex >= 0) {
        tab.inputValue = val.slice(0, atIndex);
        // Force reactive update
        tabs = [...tabs];
      }
      
      closeAtMenu();
      focusInput();
    } catch (e) {
      console.error('Failed to read file:', e);
    }
  }

  async function saveMcpServers() {
    try {
      await invoke('mcp_save_servers', { servers: mcpServers });
    } catch (e) {
      console.error('Failed to save MCP servers:', e);
    }
  }

  function addMcpServer(name: string, serverUrl: string, authToken: string, authCommand: string) {
    const entry: McpServerEntry = { name, server_url: serverUrl };
    if (authToken) entry.auth_token = authToken;
    if (authCommand) entry.auth_command = authCommand;
    mcpServers = [...mcpServers.filter(s => s.name !== name), entry];
    saveMcpServers();
  }

  function removeMcpServer(name: string) {
    mcpServers = mcpServers.filter(s => s.name !== name);
    saveMcpServers();
  }

  function getLastPromptIndex(lines: { text: string; style: CellStyle }[][]): number {
    for (let i = lines.length - 1; i >= 0; i--) {
      const t = lines[i].map(r => r.text).join('');
      const ha = lines[i].some(r => r.style.fg || r.style.bg || r.style.bold || r.style.italic || r.style.underline);
      if (t.trim() && !ha && highlightPrompt(t)) return i;
    }
    return -1;
  }

  // Smart autocomplete: command-aware file/dir/flag suggestions
  // Maps command → what kind of path argument it expects + common flags
  const CMD_COMPLETIONS: Record<string, { type: 'file' | 'dir' | 'any'; flags: string[] }> = {
    cd:    { type: 'dir',  flags: [] },
    ls:    { type: 'dir',  flags: ['-la', '-lah', '-R', '-al', '-1'] },
    cat:   { type: 'file', flags: ['-n', '-b'] },
    less:  { type: 'file', flags: ['-N', '-S'] },
    more:  { type: 'file', flags: [] },
    head:  { type: 'file', flags: ['-n 20', '-n 50'] },
    tail:  { type: 'file', flags: ['-n 20', '-f', '-n 50'] },
    rm:    { type: 'any',  flags: ['-r', '-rf', '-i'] },
    cp:    { type: 'any',  flags: ['-r', '-rv', '-i'] },
    mv:    { type: 'any',  flags: ['-i', '-v'] },
    mkdir: { type: 'dir',  flags: ['-p'] },
    rmdir: { type: 'dir',  flags: [] },
    touch: { type: 'file', flags: [] },
    chmod: { type: 'any',  flags: ['-R'] },
    chown: { type: 'any',  flags: ['-R'] },
    vim:   { type: 'file', flags: [] },
    nvim:  { type: 'file', flags: [] },
    nano:  { type: 'file', flags: [] },
    code:  { type: 'any',  flags: ['.', '-r'] },
    grep:  { type: 'file', flags: ['-r', '-rn', '-ri', '-rl', '--include'] },
    find:  { type: 'dir',  flags: ['-name', '-type f', '-type d'] },
    wc:    { type: 'file', flags: ['-l', '-w', '-c'] },
    diff:  { type: 'file', flags: ['-u', '--color'] },
    tar:   { type: 'file', flags: ['-xzf', '-czf', '-xvf', '-tvf'] },
    unzip: { type: 'file', flags: ['-l'] },
    file:  { type: 'file', flags: [] },
    stat:  { type: 'any',  flags: [] },
    du:    { type: 'dir',  flags: ['-sh', '-sh *', '-d 1'] },
    open:  { type: 'any',  flags: [] },
    source:{ type: 'file', flags: [] },
    bat:   { type: 'file', flags: ['-l', '--theme'] },
    rg:    { type: 'file', flags: ['-i', '-l', '--hidden', '-g'] },
  };

  // Cache directory listings to avoid repeated Rust calls
  let dirCache = new Map<string, { entries: FileEntry[]; ts: number }>();
  const DIR_CACHE_TTL = 5000; // 5 seconds
  let smartCompleteTimer: ReturnType<typeof setTimeout> | null = null;
  let smartCompleteVersion = 0; // monotonic counter to discard stale results

  async function getDirEntries(dirPath: string): Promise<FileEntry[]> {
    const cached = dirCache.get(dirPath);
    if (cached && Date.now() - cached.ts < DIR_CACHE_TTL) return cached.entries;
    try {
      const entries: FileEntry[] = await invoke('list_directory', { path: dirPath });
      dirCache.set(dirPath, { entries, ts: Date.now() });
      return entries;
    } catch {
      return [];
    }
  }

  function parseInputForCompletion(input: string): { cmd: string; prefix: string; beforePrefix: string } | null {
    const trimmed = input.trimStart();
    const spaceIdx = trimmed.indexOf(' ');
    if (spaceIdx === -1) return null; // Still typing the command itself
    const cmd = trimmed.slice(0, spaceIdx);
    const rest = trimmed.slice(spaceIdx + 1);
    // Find the last "token" being typed (after the last space)
    const lastSpaceIdx = rest.lastIndexOf(' ');
    const prefix = lastSpaceIdx === -1 ? rest : rest.slice(lastSpaceIdx + 1);
    const beforePrefix = input.slice(0, input.length - prefix.length);
    return { cmd, prefix, beforePrefix };
  }

  async function updateAutocompleteSuggestion(input: string) {
    if (!input || input.length < 1) {
      autocompleteSuggestion = '';
      return;
    }

    const tab = focusedTab();
    const parsed = parseInputForCompletion(input);

    // If we haven't typed a space yet, fall back to history + command name completion
    if (!parsed) {
      const lower = input.toLowerCase();
      // Try matching command names first
      const cmdMatch = Object.keys(CMD_COMPLETIONS).find(c => c.startsWith(lower) && c !== lower);
      if (cmdMatch) {
        autocompleteSuggestion = cmdMatch;
        return;
      }
      // Fall back to history
      const histMatch = globalHistory.find(h => h.toLowerCase().startsWith(lower) && h.toLowerCase() !== lower);
      autocompleteSuggestion = histMatch || '';
      return;
    }

    const { cmd, prefix, beforePrefix } = parsed;
    const cmdInfo = CMD_COMPLETIONS[cmd];

    // If the prefix starts with '-', suggest flags
    if (prefix.startsWith('-') && cmdInfo?.flags.length) {
      const flagMatch = cmdInfo.flags.find(f => f.startsWith(prefix) && f !== prefix);
      if (flagMatch) {
        autocompleteSuggestion = beforePrefix + flagMatch;
        return;
      }
    }

    // If we know this command and the user is typing a path argument, do file completion
    if (cmdInfo && tab) {
      const cwd = tab.cwd || '~';
      // Determine which directory to list and what partial name to match
      let dirToList = cwd;
      let partialName = prefix;

      if (prefix.includes('/')) {
        const lastSlash = prefix.lastIndexOf('/');
        const dirPart = prefix.slice(0, lastSlash + 1);
        partialName = prefix.slice(lastSlash + 1);
        // Resolve relative to cwd
        if (dirPart.startsWith('/') || dirPart.startsWith('~')) {
          dirToList = dirPart;
        } else {
          dirToList = cwd.endsWith('/') ? cwd + dirPart : cwd + '/' + dirPart;
        }
      }

      // Only fetch if we have at least 1 char to match (or empty prefix after a slash)
      if (partialName.length > 0 || prefix.endsWith('/')) {
        try {
          const entries = await getDirEntries(dirToList);
          const lower = partialName.toLowerCase();
          // Filter by type preference and prefix
          const matches = entries.filter(e => {
            if (!e.name.toLowerCase().startsWith(lower)) return false;
            if (e.name.toLowerCase() === lower) return false; // exact match, no suggestion
            if (e.is_hidden && !partialName.startsWith('.')) return false;
            if (cmdInfo.type === 'dir' && !e.is_dir) return false;
            if (cmdInfo.type === 'file' && e.is_dir) return false;
            return true;
          });

          if (matches.length > 0) {
            // Pick the best match (prefer exact case prefix, then first alphabetically)
            const best = matches.find(e => e.name.startsWith(partialName)) || matches[0];
            const suffix = best.is_dir ? '/' : '';
            // Reconstruct the suggestion: everything before the partial + completed name
            const dirPrefix = prefix.includes('/') ? prefix.slice(0, prefix.lastIndexOf('/') + 1) : '';
            autocompleteSuggestion = beforePrefix + dirPrefix + best.name + suffix;
            return;
          }
        } catch { /* fall through to history */ }
      }
    }

    // Fall back to history-based suggestion
    const lower = input.toLowerCase();
    const histMatch = globalHistory.find(h => h.toLowerCase().startsWith(lower) && h.toLowerCase() !== lower);
    autocompleteSuggestion = histMatch || '';
  }

  function debouncedSmartComplete(input: string) {
    // Clear any pending async lookup
    if (smartCompleteTimer) clearTimeout(smartCompleteTimer);

    if (!input || input.length < 1) {
      autocompleteSuggestion = '';
      return;
    }

    const parsed = parseInputForCompletion(input);

    // Synchronous path: command name or history completion (no space typed yet)
    if (!parsed) {
      const lower = input.toLowerCase();
      const cmdMatch = Object.keys(CMD_COMPLETIONS).find(c => c.startsWith(lower) && c !== lower);
      if (cmdMatch) { autocompleteSuggestion = cmdMatch; return; }
      const histMatch = globalHistory.find(h => h.toLowerCase().startsWith(lower) && h.toLowerCase() !== lower);
      autocompleteSuggestion = histMatch || '';
      return;
    }

    // Synchronous path: flag completion
    const { cmd, prefix, beforePrefix } = parsed;
    const cmdInfo = CMD_COMPLETIONS[cmd];
    if (prefix.startsWith('-') && cmdInfo?.flags.length) {
      const flagMatch = cmdInfo.flags.find(f => f.startsWith(prefix) && f !== prefix);
      if (flagMatch) { autocompleteSuggestion = beforePrefix + flagMatch; return; }
    }

    // Async path: file/dir completion — debounce 150ms to avoid hammering FS
    const version = ++smartCompleteVersion;
    smartCompleteTimer = setTimeout(async () => {
      await updateAutocompleteSuggestion(input);
      // Discard if a newer keystroke has fired since
      if (smartCompleteVersion !== version) return;
    }, 150);
  }

  function acceptAutocomplete() {
    if (autocompleteSuggestion) {
      const tab = focusedTab();
      if (tab) {
        tab.inputValue = autocompleteSuggestion;
        autocompleteSuggestion = '';
      }
    }
  }

  async function connectSshSession(session: SshSession) {
    sshDropdownOpen = false;
    const tab = focusedTab();
    if (!tab || !tab.connected) return;
    tab.inputValue = '';
    try {
      await invoke('write_to_shell', { sessionId: tab.sessionId, data: session.command + '\r' });
    } catch (e) {
      console.error('Failed to run SSH command:', e);
    }
    await tick();
    scrollToBottom();
    setTimeout(() => updateTabState(tab), 1500);
  }

  async function refreshAiSessions() {
    if (!aiAccountId || !aiApiToken) {
      aiSessions = [];
      aiSessionsError = '';
      aiSessionActionError = '';
      return;
    }
    aiSessionsLoading = true;
    aiSessionsError = '';
    try {
      aiSessions = await invoke<AiSessionSummary[]>('ai_list_sessions', {
        baseUrl: aiAccountId,
        apiToken: aiApiToken,
      });
    } catch (e) {
      aiSessions = [];
      aiSessionsError = String(e);
    } finally {
      aiSessionsLoading = false;
    }
  }

  function getFilteredAiSessions(): AiSessionSummary[] {
    const query = aiSessionSearch.trim().toLowerCase();
    if (!query) return aiSessions;
    return aiSessions.filter((session) => {
      const haystacks = [
        session.title || '',
        session.lastUserMessage || '',
        session.lastAssistantMessage || '',
        session.model || '',
      ];
      return haystacks.some((value) => value.toLowerCase().includes(query));
    });
  }

  function clearAiBlocksForTab(tabId: string) {
    aiBlocks = aiBlocks.filter((block) => block.tabId !== tabId);
  }

  async function loadAiSessionDetail(sessionId: string): Promise<AiSessionDetail> {
    return kuratchiRequest<AiSessionDetail>(`/api/v1/ai/sessions/${sessionId}`);
  }

  function restoreAiSessionIntoTab(tabId: string, detail: AiSessionDetail) {
    clearAiBlocksForTab(tabId);

    const state = detail.state;
    if (!state) return;

    const restoredBlock = createAiBlock(tabId, {
      prompt: state.prompt || detail.lastUserMessage || '',
      attachments: [],
      phase: mapLivePhase(state.phase),
      commands: state.commands || [],
      toolCalls: [],
      error: state.error || '',
      raw: state.raw || '',
      response: state.response || detail.lastAssistantMessage || '',
    });

    if (restoredBlock.phase !== 'done' && restoredBlock.phase !== 'error' && restoredBlock.phase !== 'answered') {
      activeBlockId = restoredBlock.id;
    }
  }

  async function createAiSession(title?: string) {
    const session = await invoke<AiSessionSummary>('ai_create_session', {
      baseUrl: aiAccountId,
      apiToken: aiApiToken,
      title: title || null,
      model: selectedAiModel,
    });
    currentAiSessionId = session.id;
    await refreshAiSessions();
    return session;
  }

  async function resumeAiSession(session: AiSessionSummary) {
    const tab = focusedTab();
    if (!tab) return;
    if (liveAgentClient && currentAiSessionId !== session.id) {
      liveAgentClient.close();
      liveAgentClient = null;
      liveAgentSessionName = '';
    }
    currentAiSessionId = session.id;
    selectedAiModel = session.model || selectedAiModel;
    agenticMode = true;
    aiSessionsSidebarOpen = true;
    aiSessionActionError = '';
    try {
      const detail = await loadAiSessionDetail(session.id);
      restoreAiSessionIntoTab(tab.id, detail);
      await ensureLiveAgentConnection();
    } catch (e) {
      aiSessionActionError = String(e);
    }
    tick().then(() => focusInput());
  }

  function startNewAiSession() {
    currentAiSessionId = '';
    if (liveAgentClient) {
      liveAgentClient.close();
      liveAgentClient = null;
      liveAgentSessionName = '';
    }
    aiSessionsSidebarOpen = true;
    aiSessionSearch = '';
    aiSessionActionError = '';
    const tab = focusedTab();
    if (tab) clearAiBlocksForTab(tab.id);
    tick().then(() => focusInput());
  }

  function normalizeBaseUrl(value: string): string {
    const trimmed = value.trim();
    return trimmed.endsWith('/') ? trimmed.slice(0, -1) : trimmed;
  }

  async function kuratchiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(`${normalizeBaseUrl(aiAccountId)}${path}`, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${aiApiToken}`,
        ...(init.headers || {}),
      },
    });

    const payload = await response.json().catch(() => null) as { success?: boolean; data?: T; error?: string } | null;
    if (!response.ok || !payload?.success) {
      throw new Error(payload?.error || `Request failed with ${response.status}`);
    }
    return payload.data as T;
  }

  function mapLivePhase(phase: LiveIdeState['phase']): AiBlock['phase'] {
    switch (phase) {
      case 'thinking': return 'thinking';
      case 'approval': return 'commands';
      case 'executing': return 'executing';
      case 'needs_input': return 'needs_input';
      case 'answered': return 'answered';
      case 'error': return 'error';
      default: return 'done';
    }
  }

  function syncAiBlockFromLiveState(state: LiveIdeState) {
    const blockId = liveAgentActiveBlockId;
    if (!blockId) return;
    const existingBlock = aiBlocks.find((block) => block.id === blockId);
    const previousPhase = existingBlock?.phase ?? null;
    updateAiBlock(blockId, {
      prompt: state.prompt || existingBlock?.prompt || '',
      phase: mapLivePhase(state.phase),
      commands: state.commands || [],
      raw: state.raw || '',
      response: state.response || '',
      error: state.error || '',
    });
    if (state.sessionId) currentAiSessionId = state.sessionId;
    if (state.phase === 'approval' && aiExecMode === 'auto' && previousPhase !== 'commands') {
      pendingCommand = state.commands?.[0]?.text || '';
      activeBlockId = blockId;
      void executePendingCommand();
    }
  }

  function buildLivePromptOptions(tab: Tab) {
    return {
      model: selectedAiModel,
      cwd: tab.cwd || null,
      osInfo: tab.sshTarget ? (tab.remoteOs || 'Linux') : (navigator.platform || null),
      shell: tab.sshTarget ? (tab.remoteShell || 'bash') : getLocalShellLabel(),
      sshTarget: tab.sshTarget || null,
      attachments: aiAttachments.map((attachment) => ({
        name: attachment.name,
        content: attachment.content,
        mimeType: attachment.mimeType,
      })),
    };
  }

  async function ensureLiveAgentConnection(): Promise<AgentClient<LiveIdeState>> {
    if (liveAgentClient && liveAgentSessionName && currentAiSessionId && liveAgentSessionName.endsWith(`:${currentAiSessionId}`)) {
      return liveAgentClient;
    }

    liveAgentConnecting = true;
    try {
      const connection = await kuratchiRequest<LiveAgentConnectResponse>('/api/v1/ai/ide/connect', {
        method: 'POST',
        body: JSON.stringify({
          sessionId: currentAiSessionId || null,
          model: selectedAiModel,
        }),
      });

      currentAiSessionId = connection.session.id;
      if (liveAgentClient) liveAgentClient.close();

      liveAgentClient = new AgentClient<LiveIdeState>({
        agent: connection.agent.agent,
        name: connection.agent.name,
        host: connection.agent.host,
        protocol: connection.agent.protocol,
        query: connection.agent.query,
        onIdentity: (name) => {
          liveAgentSessionName = name;
        },
        onStateUpdate: (state) => {
          syncAiBlockFromLiveState(state);
        },
      });
      await Promise.race([
        liveAgentClient.ready,
        new Promise<never>((_, reject) => {
          window.setTimeout(() => reject(new Error('Timed out connecting to Kuratchi live agent')), 8000);
        }),
      ]);
      liveAgentSessionName = liveAgentClient.name;
      await refreshAiSessions();
      return liveAgentClient;
    } finally {
      liveAgentConnecting = false;
    }
  }

  async function saveSettings(accountId: string, apiToken: string) {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      await store.set('kuratchi_base_url', accountId);
      await store.set('ai_account_id', accountId);
      await store.save();
      await strongholdInsert('kuratchi_api_token', apiToken);
      await strongholdInsert('ai_api_token', apiToken);
      aiAccountId = accountId;
      aiApiToken = apiToken;
      await refreshAiSessions();
    } catch (e) {
      console.error('Failed to save settings:', e);
    }
  }

  async function revokeCredentials() {
    try {
      const store = await load('settings.json', { autoSave: true, defaults: {} });
      await store.set('kuratchi_base_url', '');
      await store.set('ai_account_id', '');
      await store.save();
      await strongholdRemove('kuratchi_api_token');
      await strongholdRemove('ai_api_token');
      aiAccountId = '';
      aiApiToken = '';
      aiSessions = [];
      currentAiSessionId = '';
      agenticMode = false;
    } catch (e) {
      console.error('Failed to revoke credentials:', e);
    }
  }

  async function saveGeminiKey(apiKey: string) {
    try {
      await strongholdInsert('gemini_api_key', apiKey);
      geminiApiKey = apiKey;
    } catch (e) {
      console.error('Failed to save Gemini key:', e);
    }
  }

  async function revokeGeminiKey() {
    try {
      await strongholdRemove('gemini_api_key');
      geminiApiKey = '';
      // If currently using a Gemini model, switch back to default
      if (getSelectedModelProvider() === 'gemini') {
        selectedAiModel = '@cf/meta/llama-4-scout-17b-16e-instruct';
      }
    } catch (e) {
      console.error('Failed to revoke Gemini key:', e);
    }
  }

  function openSettings() {
    if (document.startViewTransition) {
      document.startViewTransition(() => {
        flushSync(() => {
          currentView = 'settings';
        });
      });
    } else {
      currentView = 'settings';
    }
  }

  function closeSettings() {
    if (document.startViewTransition) {
      document.startViewTransition(() => {
        flushSync(() => {
          currentView = 'terminal';
        });
      });
    } else {
      currentView = 'terminal';
    }
    tick().then(() => focusInput());
  }

  async function openFile(entry: FileEntry) {
    if (entry.is_dir) return;
    const sshTarget = focusedTab()?.sshTarget ?? '';
    if (isImageFile(entry.name)) {
      await openImagePreview(entry.path, entry.name, sshTarget);
      return;
    }
    if (isBinaryFile(entry.name)) return;
    editorSshTarget = sshTarget;
    rightPane = 'editor';
    activeTool = null;
    await tick();
    editorViewRef?.openFile(entry.path, entry.name);
  }

  function closeEditor() {
    rightPane = null;
    editorFilePath = '';
    editorFileName = '';
    editorContent = '';
    editorError = '';
    editorDirty = false;
    editorSaveStatus = '';
    editorLang = 'text';
    editorLoading = false;
    editorSshTarget = '';
    tick().then(() => focusInput());
  }

  async function previewFile(entry: FileEntry) {
    if (entry.is_dir) return;
    const sshTarget = focusedTab()?.sshTarget ?? '';
    mdPreviewSshTarget = sshTarget;
    rightPane = 'markdown';
    activeTool = null;
    await tick();
    mdPreviewRef?.openFile(entry.path, entry.name);
  }

  async function addFileToChat(entry: FileEntry) {
    if (entry.is_dir) return;
    const tab = focusedTab();
    if (!tab) return;
    
    try {
      // Read file content
      let content: string;
      if (tab.sshTarget) {
        content = await invoke<string>('read_remote_file', { sshTarget: tab.sshTarget, path: entry.path });
      } else {
        content = await invoke<string>('read_file_contents', { path: entry.path });
      }
      
      // Determine MIME type from extension
      const ext = entry.name.split('.').pop()?.toLowerCase() || '';
      const mimeMap: Record<string, string> = {
        ts: 'text/typescript', tsx: 'text/typescript',
        js: 'text/javascript', jsx: 'text/javascript',
        rs: 'text/rust', py: 'text/python', go: 'text/go',
        svelte: 'text/svelte', vue: 'text/vue',
        html: 'text/html', css: 'text/css', scss: 'text/scss',
        json: 'application/json', yaml: 'text/yaml', yml: 'text/yaml',
        toml: 'text/toml', md: 'text/markdown', mdx: 'text/markdown',
        sh: 'text/x-shellscript', bash: 'text/x-shellscript',
        sql: 'text/sql', xml: 'text/xml', txt: 'text/plain',
      };
      const mimeType = mimeMap[ext] || 'text/plain';
      
      // Add to attachments (avoid duplicates)
      const existing = aiAttachments.find(a => a.path === entry.path);
      if (!existing) {
        aiAttachments = [...aiAttachments, {
          name: entry.name,
          path: entry.path,
          content,
          mimeType,
        }];
      }
      
      // Focus the input
      focusInput();
    } catch (e) {
      console.error('Failed to add file to chat:', e);
    }
  }

  function closeMarkdownPreview() {
    rightPane = null;
    mdPreviewFilePath = '';
    mdPreviewFileName = '';
    mdPreviewError = '';
    mdPreviewLoading = false;
    mdPreviewSshTarget = '';
    tick().then(() => focusInput());
  }

  function openMarkdownInEditor() {
    const path = mdPreviewFilePath;
    const name = mdPreviewFileName;
    const ssh = mdPreviewSshTarget;
    closeMarkdownPreview();
    editorSshTarget = ssh;
    rightPane = 'editor';
    activeTool = null;
    tick().then(() => editorViewRef?.openFile(path, name));
  }

  function switchToMarkdownPreview() {
    const path = editorFilePath;
    const name = editorFileName;
    const ssh = editorSshTarget;
    closeEditor();
    mdPreviewSshTarget = ssh;
    rightPane = 'markdown';
    activeTool = null;
    tick().then(() => mdPreviewRef?.openFile(path, name));
  }

  function openDiffViewer() {
    rightPane = 'diff';
    activeTool = null;
  }

  function closeDiffViewer() {
    rightPane = null;
    pendingCodeEdit = null;
    applyingCodeEdit = false;
    tick().then(() => focusInput());
  }

  async function applyCodeEdit() {
    if (!pendingCodeEdit) return;
    
    applyingCodeEdit = true;
    try {
      const filePath = pendingCodeEdit.file_path;
      if (filePath) {
        // Write to file system
        await invoke('write_file_contents', {
          path: filePath,
          contents: pendingCodeEdit.modified,
        });
        console.log('[AI Edit] Applied changes to:', filePath);
      } else {
        // No file path - copy to clipboard instead
        await navigator.clipboard.writeText(pendingCodeEdit.modified);
        console.log('[AI Edit] Copied modified code to clipboard');
      }
      closeDiffViewer();
    } catch (e) {
      console.error('[AI Edit] Failed to apply changes:', e);
      applyingCodeEdit = false;
    }
  }

  function rejectCodeEdit() {
    closeDiffViewer();
  }

  // Clickable file detection in terminal output
  const FILE_EXT_RE = /\.\w{1,10}$/;
  const PROMPT_CHARS = /[#$%>]$/;
  const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tiff', 'heic']);
  const BINARY_EXTS = new Set(['pdf', 'zip', 'tar', 'gz', 'bz2', 'xz', '7z', 'rar', 'exe', 'bin', 'so', 'dylib', 'o', 'a', 'class', 'jar', 'war', 'woff', 'woff2', 'ttf', 'eot', 'mp3', 'mp4', 'avi', 'mov', 'mkv', 'flac', 'wav']);

  function looksLikeFile(text: string): boolean {
    const t = text.trim();
    if (!t || t.includes(' ') || t.startsWith('-')) return false;
    if (PROMPT_CHARS.test(t)) return false;
    if (!FILE_EXT_RE.test(t)) return false;
    return true;
  }

  function looksLikeFileOrDir(text: string): boolean {
    const t = text.trim();
    if (!t || t.includes(' ') || t.startsWith('-')) return false;
    if (PROMPT_CHARS.test(t)) return false;
    if (FILE_EXT_RE.test(t)) return true;
    if (t.endsWith('/')) return true;
    return false;
  }

  function getWordAtOffset(text: string, offsetX: number, el: HTMLElement): string | null {
    const tokens = text.split(/(\s+)/);
    const span = document.createElement('span');
    span.style.cssText = window.getComputedStyle(el).cssText;
    span.style.position = 'absolute';
    span.style.visibility = 'hidden';
    span.style.whiteSpace = 'pre';
    document.body.appendChild(span);
    let pos = 0;
    let found: string | null = null;
    for (const token of tokens) {
      span.textContent = text.slice(0, pos + token.length);
      const endX = span.offsetWidth;
      span.textContent = text.slice(0, pos);
      const startX = span.offsetWidth;
      if (offsetX >= startX && offsetX < endX && token.trim()) {
        found = token.trim();
        break;
      }
      pos += token.length;
    }
    document.body.removeChild(span);
    return found;
  }

  let fileHoverEl: HTMLElement | null = null;

  function clearFileHover() {
    if (fileHoverEl) {
      fileHoverEl.remove();
      fileHoverEl = null;
    }
  }

  function handleTerminalMouseMove(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target || !target.classList.contains('terminal-run')) {
      clearFileHover();
      return;
    }
    const text = target.textContent || '';
    const rect = target.getBoundingClientRect();
    const offsetX = e.clientX - rect.left;

    // Single-file span
    const trimmed = text.trim();
    let word: string | null = null;
    if (looksLikeFile(trimmed) || looksLikeFileOrDir(trimmed)) {
      word = trimmed;
    } else {
      // Multi-file span: find word under cursor
      word = getWordAtOffset(text, offsetX, target);
      if (word && !looksLikeFile(word) && !looksLikeFileOrDir(word)) {
        word = null;
      }
    }

    if (!word) {
      clearFileHover();
      target.style.cursor = '';
      return;
    }

    target.style.cursor = 'pointer';

    // Position underline overlay on the exact word
    const wordStart = text.indexOf(word);
    if (wordStart === -1) { clearFileHover(); return; }

    const range = document.createRange();
    const textNode = target.firstChild;
    if (!textNode || textNode.nodeType !== Node.TEXT_NODE) { clearFileHover(); return; }
    try {
      range.setStart(textNode, wordStart);
      range.setEnd(textNode, wordStart + word.length);
    } catch {
      clearFileHover();
      return;
    }
    const wordRect = range.getBoundingClientRect();

    if (!fileHoverEl) {
      fileHoverEl = document.createElement('div');
      fileHoverEl.className = 'terminal-file-hover';
      document.body.appendChild(fileHoverEl);
    }
    fileHoverEl.style.left = `${wordRect.left}px`;
    fileHoverEl.style.top = `${wordRect.bottom - 1}px`;
    fileHoverEl.style.width = `${wordRect.width}px`;
  }

  function handleTerminalMouseLeave() {
    clearFileHover();
  }

  function getFileExt(name: string): string {
    const dot = name.lastIndexOf('.');
    return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
  }

  function isImageFile(name: string): boolean {
    return IMAGE_EXTS.has(getFileExt(name));
  }

  function isBinaryFile(name: string): boolean {
    const ext = getFileExt(name);
    return BINARY_EXTS.has(ext) || IMAGE_EXTS.has(ext);
  }

  const MD_EXTS = new Set(['md', 'mdx']);
  function isMarkdownFile(name: string): boolean {
    return MD_EXTS.has(getFileExt(name));
  }

  // Preview state for images
  let previewUrl = $state('');
  let previewFileName = $state('');
  let previewSshTarget = $state('');
  let previewLoading = $state(false);
  

  function closePreview() {
    transition(() => {
      rightPane = null;
      previewUrl = '';
      previewFileName = '';
      previewSshTarget = '';
      previewLoading = false;
    });
    tick().then(() => focusInput());
  }

  async function openImagePreview(fullPath: string, fileName: string, sshTarget: string) {
    transition(() => {
      previewFileName = fileName;
      previewSshTarget = sshTarget;
      previewLoading = true;
      rightPane = 'preview';
      activeTool = null;
    });
    try {
      if (sshTarget) {
        // Read remote image as base64
        const b64: string = await invoke('read_remote_file_base64', { sshTarget, path: fullPath });
        const ext = getFileExt(fileName);
        const mime = ext === 'svg' ? 'image/svg+xml' : `image/${ext === 'jpg' ? 'jpeg' : ext}`;
        previewUrl = `data:${mime};base64,${b64}`;
      } else {
        // Local file — use asset protocol or read as base64
        const b64: string = await invoke('read_file_base64', { path: fullPath });
        const ext = getFileExt(fileName);
        const mime = ext === 'svg' ? 'image/svg+xml' : `image/${ext === 'jpg' ? 'jpeg' : ext}`;
        previewUrl = `data:${mime};base64,${b64}`;
      }
    } catch (e) {
      previewUrl = '';
      console.error('Failed to load preview:', e);
    } finally {
      previewLoading = false;
    }
  }

  async function handleTerminalFileClick(e: MouseEvent, tab: Tab) {
    const target = e.target as HTMLElement;
    if (!target || !target.classList.contains('terminal-run')) return;
    const rawText = target.textContent || '';
    let text = rawText.trim();

    if (text && !looksLikeFile(text)) {
      const rect = target.getBoundingClientRect();
      const offsetX = e.clientX - rect.left;
      const word = getWordAtOffset(rawText, offsetX, target);
      if (word && looksLikeFile(word)) {
        text = word;
      } else {
        return;
      }
    }
    if (!text) return;

    e.stopPropagation();

    const cwd = tab.cwd || '~';
    const fullPath = text.startsWith('/') ? text : (cwd.endsWith('/') ? cwd + text : cwd + '/' + text);
    const fileName = text.split('/').pop() || text;

    if (isImageFile(fileName)) {
      await openImagePreview(fullPath, fileName, tab.sshTarget);
      return;
    }

    if (isBinaryFile(fileName)) {
      return; // Skip binary files that can't be previewed
    }

    editorSshTarget = tab.sshTarget;
    rightPane = 'editor';
    activeTool = null;
    await tick();
    editorViewRef?.openFile(fullPath, fileName);
  }

  // Terminal file context menu state
  let terminalFileCtxMenu: { x: number; y: number; filePath: string; fileName: string; tab: Tab } | null = $state(null);

  function handleTerminalFileContextMenu(e: MouseEvent, tab: Tab) {
    const target = e.target as HTMLElement;
    if (!target || !target.classList.contains('terminal-run')) return;
    const rawText = target.textContent || '';
    let text = rawText.trim();

    if (text && !looksLikeFile(text)) {
      const rect = target.getBoundingClientRect();
      const offsetX = e.clientX - rect.left;
      const word = getWordAtOffset(rawText, offsetX, target);
      if (word && looksLikeFile(word)) {
        text = word;
      } else {
        return;
      }
    }
    if (!text) return;

    e.preventDefault();
    e.stopPropagation();

    const cwd = tab.cwd || '~';
    const fullPath = text.startsWith('/') ? text : (cwd.endsWith('/') ? cwd + text : cwd + '/' + text);
    const fileName = text.split('/').pop() || text;

    terminalFileCtxMenu = { x: e.clientX, y: e.clientY, filePath: fullPath, fileName, tab };
  }

  function closeTerminalFileCtxMenu() {
    terminalFileCtxMenu = null;
  }

  async function addTerminalFileToAgent() {
    if (!terminalFileCtxMenu) return;
    const { filePath, fileName, tab } = terminalFileCtxMenu;
    closeTerminalFileCtxMenu();

    try {
      let content: string;
      if (tab.sshTarget) {
        content = await invoke<string>('read_remote_file', { sshTarget: tab.sshTarget, path: filePath });
      } else {
        content = await invoke<string>('read_file_contents', { path: filePath });
      }

      const ext = fileName.split('.').pop()?.toLowerCase() || '';
      const mimeMap: Record<string, string> = {
        ts: 'text/typescript', tsx: 'text/typescript',
        js: 'text/javascript', jsx: 'text/javascript',
        rs: 'text/rust', py: 'text/python', go: 'text/go',
        svelte: 'text/svelte', vue: 'text/vue',
        html: 'text/html', css: 'text/css', scss: 'text/scss',
        json: 'application/json', yaml: 'text/yaml', yml: 'text/yaml',
        toml: 'text/toml', md: 'text/markdown', mdx: 'text/markdown',
        sh: 'text/x-shellscript', bash: 'text/x-shellscript',
        sql: 'text/sql', xml: 'text/xml', txt: 'text/plain',
      };
      const mimeType = mimeMap[ext] || 'text/plain';

      const existing = aiAttachments.find(a => a.path === filePath);
      if (!existing) {
        aiAttachments = [...aiAttachments, {
          name: fileName,
          path: filePath,
          content,
          mimeType,
        }];
      }

      // Switch to agent mode if not already
      if (!agenticMode) {
        if (!aiAccountId || !aiApiToken) {
          openSettings();
          return;
        }
        agenticMode = true;
      }

      focusInput();
    } catch (e) {
      console.error('Failed to add file to agent:', e);
    }
  }

  // AI overlay — multiple blocks per tab (history + active)
  let aiBlockIdCounter = 0;
  interface AiCmdEntry { text: string; status: 'pending' | 'running' | 'done' | 'error' | 'input'; output: string }
  interface AiToolCallEntry { serverName: string; toolName: string; arguments: string; status: 'pending' | 'running' | 'done' | 'error'; output: string }
  interface AiBlockAttachment {
    name: string;
    content: string;  // base64 for images
    mimeType: string;
  }
  interface AiBlock {
    id: number;
    tabId: string;
    prompt: string;
    attachments: AiBlockAttachment[];  // Store images/files with the prompt
    phase: 'choosing' | 'thinking' | 'commands' | 'executing' | 'retrying' | 'needs_input' | 'done' | 'error' | 'answered' | 'passthrough';
    commands: AiCmdEntry[];
    toolCalls: AiToolCallEntry[];
    error: string;
    raw: string;
    response: string;
  }
  let aiBlocks: AiBlock[] = $state([]);
  let aiExecDropdownOpen = $state(false);

  function getAiBlocks(tabId: string): AiBlock[] {
    return aiBlocks.filter(b => b.tabId === tabId);
  }

  function getVisibleAiBlocks(tabId: string): AiBlock[] {
    const blocks = getAiBlocks(tabId);
    if (agenticMode) return blocks;
    return blocks.filter(b => b.phase !== 'done' && b.phase !== 'error' && b.phase !== 'answered');
  }

  function getActiveAiBlock(tabId: string): AiBlock | null {
    return aiBlocks.findLast(b => b.tabId === tabId && b.phase !== 'done' && b.phase !== 'error' && b.phase !== 'answered' && b.phase !== 'passthrough') ?? null;
  }
  function createAiBlock(tabId: string, data: Partial<AiBlock>): AiBlock {
    const block: AiBlock = { 
      id: ++aiBlockIdCounter, 
      tabId, 
      prompt: '', 
      attachments: data.attachments ?? [], 
      phase: 'thinking', 
      commands: [], 
      toolCalls: [], 
      error: '', 
      raw: '', 
      response: '', 
      ...data 
    };
    aiBlocks = [...aiBlocks, block];
    return block;
  }
  function updateAiBlock(blockId: number, patch: Partial<AiBlock>) {
    const idx = aiBlocks.findIndex(b => b.id === blockId);
    if (idx >= 0) {
      aiBlocks[idx] = { ...aiBlocks[idx], ...patch };
      aiBlocks = [...aiBlocks];
    }
  }
  function removeAiBlock(blockId: number) {
    aiBlocks = aiBlocks.filter(b => b.id !== blockId);
    aiExecDropdownOpen = false;
  }

  function renderAiAnswer(response: string): string {
    return renderMarkdown(response.trim());
  }

  function getAiExecutedCount(block: AiBlock): number {
    return block.commands.length + block.toolCalls.length;
  }

  let activeBlockId = $state(0);

  function chooseAiExecMode(mode: 'auto' | 'manual') {
    const block = aiBlocks.find(b => b.id === activeBlockId);
    const stashedPrompt = block?.prompt || '';
    aiExecMode = mode;
    aiExecPromptShown = false;
    pendingCommand = '';
    if (block) removeAiBlock(block.id);
    const tab = focusedTab();
    if (tab && stashedPrompt) {
      tab.inputValue = stashedPrompt;
      handleAgenticSubmit();
    }
  }

  async function handleAttachFile() {
    try {
      const selected = await invoke<string[]>('select_files_dialog');
      if (!selected || selected.length === 0) return;
      
      for (const filePath of selected) {
        const fileName = filePath.split('/').pop() || filePath;
        
        // Check if already attached
        if (aiAttachments.some(a => a.path === filePath)) continue;
        
        // Determine mime type
        const ext = getFileExt(fileName);
        let mimeType = 'text/plain';
        if (IMAGE_EXTS.has(ext)) {
          mimeType = ext === 'svg' ? 'image/svg+xml' : `image/${ext === 'jpg' ? 'jpeg' : ext}`;
        } else if (ext === 'pdf') {
          mimeType = 'application/pdf';
        } else if (ext === 'json') {
          mimeType = 'application/json';
        }
        
        // Read file content as base64
        let content: string;
        if (IMAGE_EXTS.has(ext) || ext === 'pdf') {
          content = await invoke('read_file_base64', { path: filePath });
        } else {
          // Read as text
          content = await invoke('read_file_text', { path: filePath });
        }
        
        aiAttachments = [...aiAttachments, { name: fileName, path: filePath, content, mimeType }];
      }
    } catch (e) {
      console.error('Failed to attach file:', e);
    }
  }

  function removeAttachment(index: number) {
    aiAttachments = aiAttachments.filter((_, i) => i !== index);
  }

  function clearAllAttachments() {
    aiAttachments = [];
  }

  function selectAiModel(modelId: string) {
    selectedAiModel = modelId;
    aiModelDropdownOpen = false;
  }

  function getSelectedModelName(): string {
    return AI_MODELS.find(m => m.id === selectedAiModel)?.name || 'Llama 4 Scout';
  }

  async function handlePaste(e: ClipboardEvent) {
    if (!agenticMode) return;
    
    const items = e.clipboardData?.items;
    if (!items) return;
    
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      
      // Check if it's an image
      if (item.type.startsWith('image/')) {
        e.preventDefault();
        
        const blob = item.getAsFile();
        if (!blob) continue;
        
        try {
          // Convert blob to base64
          const reader = new FileReader();
          const base64Promise = new Promise<string>((resolve, reject) => {
            reader.onloadend = () => {
              const result = reader.result as string;
              // Remove data URL prefix to get just the base64 data
              const base64 = result.split(',')[1];
              resolve(base64);
            };
            reader.onerror = reject;
          });
          
          reader.readAsDataURL(blob);
          const content = await base64Promise;
          
          // Generate a unique filename for the pasted image
          const timestamp = Date.now();
          const ext = item.type.split('/')[1] || 'png';
          const fileName = `pasted-image-${timestamp}.${ext}`;
          
          // Add to attachments
          aiAttachments = [...aiAttachments, {
            name: fileName,
            path: fileName, // Use filename as path for pasted images
            content,
            mimeType: item.type,
          }];
        } catch (err) {
          console.error('Failed to process pasted image:', err);
        }
      }
    }
  }

  async function handleAgenticSubmit() {
    const tab = focusedTab();
    if (!tab || !tab.inputValue.trim() || !tab.connected) return;

    const input = tab.inputValue.trim();

    // Passthrough: basic shell commands run inline in the conversation
    if (isBasicShellCommand(input)) {
      tab.inputValue = '';
      resetInputHeight(tab.id);
      const block = createAiBlock(tab.id, {
        prompt: input,
        phase: 'passthrough',
        commands: [{ text: input, status: 'running', output: '' }],
      });
      activeBlockId = block.id;
      scrollToBottom();

      const linesBefore = getPlainLines(tab).length;
      try {
        await invoke('write_to_shell', { sessionId: tab.sessionId, data: input + '\r' });
        const output = await waitForOutput(tab, linesBefore, 3000, true);
        block.commands[0].output = output;
        block.commands[0].status = 'done';
      } catch (e) {
        block.commands[0].output = String(e);
        block.commands[0].status = 'error';
        console.error('Failed to execute passthrough command:', e);
      }
      updateAiBlock(block.id, { commands: [...block.commands] });
      await tick();
      scrollToBottom();
      setTimeout(() => updateTabState(tab), 500);
      return;
    }

    // First-time exec mode prompt
    if (aiExecMode === null) {
      tab.inputValue = '';
      resetInputHeight(tab.id);
      aiExecPromptShown = true;
      // Store attachments with the block for history
      const blockAttachments = aiAttachments.map(a => ({ name: a.name, content: a.content, mimeType: a.mimeType }));
      const block = createAiBlock(tab.id, { prompt: input, attachments: blockAttachments, phase: 'choosing' });
      activeBlockId = block.id;
      scrollToBottom();
      return;
    }

    tab.inputValue = '';
    resetInputHeight(tab.id);
    // Store attachments with the block for history
    const blockAttachments = aiAttachments.map(a => ({ name: a.name, content: a.content, mimeType: a.mimeType }));
    const block = createAiBlock(tab.id, { prompt: input, attachments: blockAttachments, phase: 'thinking' });
    activeBlockId = block.id;
    scrollToBottom();
    aiLoading = true;

    // Check if this is a code modification request with a text file attachment
    const promptLower = input.toLowerCase();
    const isModificationRequest = promptLower.includes('add') || promptLower.includes('modify') || 
      promptLower.includes('change') || promptLower.includes('update') || promptLower.includes('fix') ||
      promptLower.includes('refactor') || promptLower.includes('implement') || promptLower.includes('create') ||
      promptLower.includes('remove') || promptLower.includes('delete') || promptLower.includes('rename');
    
    const textFileAttachment = aiAttachments.find(a => 
      !a.mimeType.startsWith('image/') && a.mimeType !== 'application/pdf'
    );
    
    // Detect file paths in prompt (e.g., src/lib/foo.ts, ./bar.js, /path/to/file.rs)
    const filePathRegex = /(?:^|\s)((?:\.{0,2}\/)?(?:[\w.-]+\/)*[\w.-]+\.(?:ts|tsx|js|jsx|rs|py|go|svelte|vue|html|css|scss|json|yaml|yml|toml|md|mdx|sh|bash|sql|xml|txt|c|cpp|h|hpp|java|kt|swift|rb|php|lua|zig))(?:\s|$|[,;:])/gi;
    const filePathMatches = input.match(filePathRegex);
    const detectedFilePath = filePathMatches ? filePathMatches[0].trim().replace(/[,;:]$/, '') : null;
    
    const provider = getSelectedModelProvider();

    // If modification request with text file attachment, use code edit flow
    if (isModificationRequest && textFileAttachment) {
      try {
        console.log('[AI] Code edit request detected for:', textFileAttachment.name);
        const editResult: CodeEditResult = provider === 'gemini'
          ? await invoke('ai_gemini_edit_code', {
              apiKey: geminiApiKey,
              model: selectedAiModel,
              fileName: textFileAttachment.name,
              filePath: textFileAttachment.path || null,
              fileContent: textFileAttachment.content,
              instruction: input,
            })
          : await invoke('ai_edit_code', {
              baseUrl: aiAccountId,
              apiToken: aiApiToken,
              model: selectedAiModel,
              fileName: textFileAttachment.name,
              filePath: textFileAttachment.path || null,
              fileContent: textFileAttachment.content,
              instruction: input,
            });
        
        // Store the pending edit and show in diff viewer
        pendingCodeEdit = editResult;
        aiAttachments = [];
        updateAiBlock(block.id, { 
          phase: 'answered', 
          response: `Code edit ready for review:\n${editResult.summary}`,
          raw: 'CODE_EDIT'
        });
        aiLoading = false;
        // Open the diff viewer pane
        rightPane = 'diff';
        activeTool = null;
        scrollToBottom();
        return;
      } catch (e) {
        updateAiBlock(block.id, { phase: 'error', error: `Code edit failed: ${e}` });
        aiLoading = false;
        scrollToBottom();
        return;
      }
    }
    
    // If modification request with file path in prompt (no attachment), read from filesystem
    if (isModificationRequest && detectedFilePath && !textFileAttachment) {
      try {
        console.log('[AI] Code edit from path detected:', detectedFilePath);
        const editResult: CodeEditResult = await invoke(
          provider === 'gemini' ? 'ai_gemini_edit_code_from_path' : 'ai_edit_code_from_path',
          provider === 'gemini'
            ? { apiKey: geminiApiKey, model: selectedAiModel, filePath: detectedFilePath, instruction: input, cwd: tab.cwd || null }
            : { baseUrl: aiAccountId, apiToken: aiApiToken, model: selectedAiModel, filePath: detectedFilePath, instruction: input, cwd: tab.cwd || null }
        );
        
        // Store the pending edit and show in diff viewer
        pendingCodeEdit = editResult;
        updateAiBlock(block.id, { 
          phase: 'answered', 
          response: `Code edit ready for review:\n${editResult.summary}`,
          raw: 'CODE_EDIT'
        });
        aiLoading = false;
        // Open the diff viewer pane
        rightPane = 'diff';
        activeTool = null;
        scrollToBottom();
        return;
      } catch (e) {
        updateAiBlock(block.id, { phase: 'error', error: `Code edit failed: ${e}` });
        aiLoading = false;
        scrollToBottom();
        return;
      }
    }

    // Live agent only works with Kuratchi (Cloudflare)
    if (provider === 'cloudflare') {
      try {
        const client = await ensureLiveAgentConnection();
        liveAgentActiveBlockId = block.id;
        await client.call('submitPrompt', [input, buildLivePromptOptions(tab)]);
        aiAttachments = [];
        aiLoading = false;
        pendingCommand = '';
        return;
      } catch (e) {
        console.error('Live agent submit failed, falling back to REST path:', e);
      }
    }

    // Step 1: Get commands/tool calls from AI (MCP-aware)
    let rawResponse: string;
    const attachmentPayload = aiAttachments.map(a => ({
      name: a.name,
      content: a.content,
      mimeType: a.mimeType,
    }));
    try {
      if (provider === 'gemini') {
        rawResponse = await invoke('ai_gemini_chat', {
          apiKey: geminiApiKey,
          model: selectedAiModel,
          prompt: input,
          attachments: attachmentPayload,
          cwd: tab.cwd || null,
          osInfo: tab.sshTarget ? (tab.remoteOs || 'Linux') : (navigator.platform || null),
          shell: tab.sshTarget ? (tab.remoteShell || 'bash') : getLocalShellLabel(),
          sshTarget: tab.sshTarget || null,
        });
      } else {
        const turn: AiChatTurn = await invoke('ai_chat_with_tools', {
          baseUrl: aiAccountId,
          apiToken: aiApiToken,
          model: selectedAiModel,
          sessionId: currentAiSessionId || null,
          prompt: input,
          attachments: attachmentPayload,
          cwd: tab.cwd || null,
          osInfo: tab.sshTarget ? (tab.remoteOs || 'Linux') : (navigator.platform || null),
          shell: tab.sshTarget ? (tab.remoteShell || 'bash') : getLocalShellLabel(),
          sshTarget: tab.sshTarget || null,
        });
        if (turn.session_id) currentAiSessionId = turn.session_id;
        await refreshAiSessions();
        rawResponse = turn.response;
      }
      aiAttachments = [];
    } catch (e) {
      updateAiBlock(block.id, { phase: 'error', error: `AI request failed: ${e}` });
      aiLoading = false;
      scrollToBottom();
      return;
    }

    // Step 2: Parse response — separate TOOL_CALL lines from shell commands
    console.log('[AI] raw response:', rawResponse);
    const rawText = rawResponse.trim();
    
    // Handle direct answers (e.g., from vision model image analysis)
    if (rawText.startsWith('ANSWER:')) {
      const answerText = rawText.substring('ANSWER:'.length).trim();
      updateAiBlock(block.id, { phase: 'answered', response: answerText, raw: rawText });
      aiLoading = false;
      scrollToBottom();
      return;
    }
    
    const allLines = rawText.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#') && !l.startsWith('ERROR:'));

    if (allLines.length === 0 || rawText.startsWith('ERROR:')) {
      updateAiBlock(block.id, { phase: 'answered', response: rawText, raw: rawText });
      aiLoading = false;
      scrollToBottom();
      return;
    }

    const toolCallLines: AiToolCallEntry[] = [];
    const shellCmdLines: string[] = [];

    for (const line of allLines) {
      if (line.startsWith('TOOL_CALL:')) {
        const parts = line.substring('TOOL_CALL:'.length).split('|').map(s => s.trim());
        if (parts.length >= 3) {
          toolCallLines.push({
            serverName: parts[0],
            toolName: parts[1],
            arguments: parts.slice(2).join('|').trim(),
            status: 'pending',
            output: '',
          });
        }
      } else {
        shellCmdLines.push(line);
      }
    }

    const commands: AiCmdEntry[] = shellCmdLines.map(c => ({ text: c, status: 'pending' as const, output: '' }));
    updateAiBlock(block.id, { commands, toolCalls: toolCallLines, raw: rawText });

    const hasToolCalls = toolCallLines.length > 0;
    const hasShellCmds = commands.length > 0;

    // Step 3: Auto-execute or show for manual approval
    if (aiExecMode === 'auto') {
      updateAiBlock(block.id, { phase: 'executing' });
      scrollToBottom();

      // Execute MCP tool calls
      for (let i = 0; i < toolCallLines.length; i++) {
        toolCallLines[i].status = 'running';
        updateAiBlock(block.id, { toolCalls: [...toolCallLines] });
        scrollToBottom();

        try {
          const servers: {name: string; server_url: string}[] = await invoke('mcp_list_servers');
          const server = servers.find(s => s.name === toolCallLines[i].serverName);
          if (!server) throw new Error(`Unknown MCP server: ${toolCallLines[i].serverName}`);

          let args: Record<string, unknown> = {};
          try { args = JSON.parse(toolCallLines[i].arguments); } catch { args = {}; }

          const result: { content: {type: string; text?: string}[]; is_error?: boolean } = await invoke('mcp_call_tool', {
            serverUrl: server.server_url,
            serverName: server.name,
            toolName: toolCallLines[i].toolName,
            arguments: args,
          });

          console.log('[mcp_call_tool] result:', JSON.stringify(result));
          const outputText = result.content
            .filter((c: {type: string; text?: string}) => c.text)
            .map((c: {type: string; text?: string}) => c.text)
            .join('\n');
          console.log('[mcp_call_tool] outputText:', outputText.substring(0, 500));

          toolCallLines[i].status = result.is_error ? 'error' : 'done';
          toolCallLines[i].output = outputText;
        } catch (e) {
          toolCallLines[i].status = 'error';
          toolCallLines[i].output = String(e);
        }
        updateAiBlock(block.id, { toolCalls: [...toolCallLines] });
        scrollToBottom();
      }

      // Execute shell commands with retry loop
      const MAX_RETRIES = 3;
      const failedAttempts: { command: string; error: string }[] = [];
      let inputPrompt: string | null = null;
      // allCommands tracks every command shown to the user (including retries)
      let allCommands: AiCmdEntry[] = [...commands];
      // pendingCmds are the ones to execute in this iteration
      let pendingCmds = commands;

      for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
        const errorCmds: { command: string; error: string }[] = [];

        for (let i = 0; i < pendingCmds.length; i++) {
          pendingCmds[i].status = 'running';
          updateAiBlock(block.id, { commands: [...allCommands] });
          scrollToBottom();

          const linesBefore = getPlainLines(tab).length;
          try {
            await invoke('write_to_shell', { sessionId: tab.sessionId, data: pendingCmds[i].text + '\r' });
            const timeout = tab.sshTarget ? 5000 : 2000;
            const output = await waitForOutput(tab, linesBefore, timeout, true);
            pendingCmds[i].output = output;

            const interactivePrompt = extractInteractivePrompt(output);
            if (tab.awaitingInput || interactivePrompt) {
              pendingCmds[i].status = 'input';
              inputPrompt = interactivePrompt || output.trim() || 'Command is waiting for input.';
              break;
            }

            if (looksLikeError(output)) {
              pendingCmds[i].status = 'error';
              errorCmds.push({ command: pendingCmds[i].text, error: output });
            } else {
              pendingCmds[i].status = 'done';
            }
          } catch (e) {
            pendingCmds[i].status = 'error';
            pendingCmds[i].output = String(e);
            errorCmds.push({ command: pendingCmds[i].text, error: String(e) });
          }
          updateAiBlock(block.id, { commands: [...allCommands] });
          scrollToBottom();
        }

        if (inputPrompt) break;

        // If no errors or exhausted retries, stop
        if (errorCmds.length === 0 || attempt === MAX_RETRIES) break;

        // Ask AI for alternative commands
        failedAttempts.push(...errorCmds);
        updateAiBlock(block.id, { phase: 'retrying' });
        scrollToBottom();
        try {
          const retryResponse: string = await invoke('ai_retry', {
            baseUrl: aiAccountId,
            apiToken: aiApiToken,
            originalPrompt: input,
            failedAttempts,
            cwd: tab.cwd || null,
            osInfo: tab.sshTarget ? (tab.remoteOs || 'Linux') : (navigator.platform || null),
            shell: tab.sshTarget ? (tab.remoteShell || 'bash') : getLocalShellLabel(),
            sshTarget: tab.sshTarget || null,
          });

          const retryText = retryResponse.trim();

          // If AI says it can't be done or answers directly, stop retrying
          if (retryText.startsWith('ERROR:') || retryText.startsWith('ANSWER:')) {
            const label = retryText.startsWith('ANSWER:') ? retryText.substring(7).trim() : retryText;
            allCommands.push({ text: '[AI] ' + label, status: 'done' as const, output: '' });
            updateAiBlock(block.id, { commands: [...allCommands] });
            break;
          }

          const retryLines = retryText.split('\n').map(l => l.trim()).filter(l => l && !l.startsWith('#') && !l.startsWith('ERROR:'));
          if (retryLines.length === 0) break;

          // Create new command entries and queue them for the next iteration
          pendingCmds = retryLines.map(c => ({ text: c, status: 'pending' as const, output: '' }));
          allCommands = [...allCommands, ...pendingCmds];
          updateAiBlock(block.id, { commands: [...allCommands], phase: 'executing' });
        } catch {
          // Retry request failed - stop
          break;
        }
      }

      if (inputPrompt) {
        updateAiBlock(block.id, {
          phase: 'needs_input',
          commands: [...allCommands],
          response: buildInteractivePromptResponse(inputPrompt),
          raw: rawText,
        });
        aiLoading = false;
        pendingCommand = '';
        scrollToBottom();
        setTimeout(() => updateTabState(tab), 500);
        return;
      }

      // Step 4: Summarize results
      updateAiBlock(block.id, { phase: 'thinking' });
      scrollToBottom();
      try {
        const toolResults = toolCallLines.map(t => ({
          server_name: t.serverName,
          tool_name: t.toolName,
          arguments: t.arguments,
          output: t.output,
          is_error: t.status === 'error',
        }));
        const realCommands = allCommands.filter(c => !c.text.startsWith('[AI]'));
        const commandResults = realCommands.map(c => ({ command: c.text, output: c.output }));

        let summary: string;
        if (hasToolCalls) {
          summary = await invoke('ai_summarize_tool_results', {
            baseUrl: aiAccountId,
            apiToken: aiApiToken,
            originalPrompt: input,
            toolResults,
            commandResults,
          });
        } else {
          summary = await invoke('ai_summarize', {
            baseUrl: aiAccountId,
            apiToken: aiApiToken,
            originalPrompt: input,
            commandResults,
          });
        }
        // Include any ANSWER/ERROR messages from retries
        const aiMessages = allCommands.filter(c => c.text.startsWith('[AI]')).map(c => c.text.substring(5));
        const fullSummary = aiMessages.length > 0
          ? aiMessages.join('\n') + '\n\n' + summary.trim()
          : summary.trim();
        updateAiBlock(block.id, { phase: 'answered', response: fullSummary, raw: rawText });
      } catch {
        updateAiBlock(block.id, { phase: 'done' });
      }
      aiLoading = false;
      pendingCommand = '';
      scrollToBottom();
      setTimeout(() => updateTabState(tab), 500);
    } else {
      // Manual mode: show commands/tool calls for user approval
      if (hasShellCmds) pendingCommand = shellCmdLines[0];
      updateAiBlock(block.id, { phase: 'commands' });
      aiLoading = false;
      scrollToBottom();
    }
  }

  function getPlainLines(tab: Tab): string[] {
    return tab.emulator.getRawLines().map(
      (row: { text: string }[]) => row.map(r => r.text).join('')
    );
  }

  async function waitForOutput(tab: Tab, prevLineCount: number, timeoutMs = 2000, hideFromTerminal = false): Promise<string> {
    const start = Date.now();
    let lastCount = prevLineCount;
    let stableTime = 0;

    while (Date.now() - start < timeoutMs) {
      await new Promise(r => setTimeout(r, 150));
      if (tab.awaitingInput) break;
      const lines = getPlainLines(tab);
      if (lines.length !== lastCount) {
        lastCount = lines.length;
        stableTime = 0;
      } else {
        stableTime += 150;
        if (stableTime >= 400) break;
      }
    }

    // Get all new lines that appeared
    const allLines = getPlainLines(tab);
    const newLines = allLines.slice(prevLineCount);
    
    // Hide the newly generated lines if requested
    // We need to hide from the line AFTER prevLineCount (the command echo) to the current last line
    if (hideFromTerminal && newLines.length > 0) {
      const startAbsoluteLine = tab.emulator.linesDropped + prevLineCount;
      const endAbsoluteLine = tab.emulator.linesDropped + allLines.length - 1;
      tab.emulator.hideLineRange(startAbsoluteLine, endAbsoluteLine);
      tab.renderVersion++;
    }

    // Create a truncated version for display in AI block
    const outputLines = [...newLines];
    
    // Remove the command echo line (first line)
    if (outputLines.length > 0) outputLines.shift();
    
    // Trim trailing empty lines and the prompt line at the end
    while (outputLines.length > 0 && outputLines[outputLines.length - 1].trim() === '') outputLines.pop();
    if (outputLines.length > 0) {
      const last = outputLines[outputLines.length - 1];
      if (last.includes('$') || last.includes('%') || last.includes('#') || /^PS .+>/.test(last.trim())) outputLines.pop();
    }
    
    // Truncate if too long (keep first 50 lines)
    const MAX_DISPLAY_LINES = 50;
    if (outputLines.length > MAX_DISPLAY_LINES) {
      const truncated = outputLines.slice(0, MAX_DISPLAY_LINES);
      truncated.push(`\n... (${outputLines.length - MAX_DISPLAY_LINES} more lines, truncated for display)`);
      return truncated.join('\n');
    }
    
    return outputLines.join('\n');
  }

  async function executeBlockInline(tab: Tab, blockId: number) {
    const block = aiBlocks.find(b => b.id === blockId);
    if (!block) return;
    const failedAttempts: { command: string; error: string }[] = [];

    for (let i = 0; i < block.commands.length; i++) {
      block.commands[i].status = 'running';
      updateAiBlock(blockId, { commands: [...block.commands] });
      scrollToBottom();

      const linesBefore = getPlainLines(tab).length;

      try {
        await invoke('write_to_shell', { sessionId: tab.sessionId, data: block.commands[i].text + '\r' });
        const output = await waitForOutput(tab, linesBefore, 2000, true);
        block.commands[i].output = output;

        const interactivePrompt = extractInteractivePrompt(output);
        if (tab.awaitingInput || interactivePrompt) {
          block.commands[i].status = 'input';
          updateAiBlock(blockId, {
            commands: [...block.commands],
            phase: 'needs_input',
            response: buildInteractivePromptResponse(interactivePrompt || output.trim() || 'Command is waiting for input.'),
          });
          pendingCommand = '';
          await tick();
          scrollToBottom();
          setTimeout(() => updateTabState(tab), 500);
          return;
        }

        block.commands[i].status = 'done';
      } catch (e) {
        block.commands[i].status = 'error';
        block.commands[i].output = String(e);
        failedAttempts.push({ command: block.commands[i].text, error: String(e) });
        console.error(`Failed to execute command ${i + 1}:`, e);
        updateAiBlock(blockId, { commands: [...block.commands] });
        break;
      }
      updateAiBlock(blockId, { commands: [...block.commands] });
      scrollToBottom();
    }

    if (liveAgentClient && currentAiSessionId) {
      try {
        if (failedAttempts.length > 0) {
          await liveAgentClient.call('retryPrompt', [block.prompt, failedAttempts, buildLivePromptOptions(tab)]);
          pendingCommand = '';
          await tick();
          scrollToBottom();
          return;
        }

        const commandResults = block.commands.map((command) => ({
          command: command.text,
          output: command.output,
        }));
        await liveAgentClient.call('submitExecutionResults', [block.prompt, commandResults]);
        pendingCommand = '';
        await tick();
        scrollToBottom();
        setTimeout(() => updateTabState(tab), 500);
        return;
      } catch (e) {
        console.error('Live execution finalize failed:', e);
      }
    }

    updateAiBlock(blockId, { phase: 'done' });
    pendingCommand = '';
    await tick();
    scrollToBottom();
    setTimeout(() => updateTabState(tab), 500);
  }

  async function executePendingCommand() {
    const tab = focusedTab();
    const block = aiBlocks.find(b => b.id === activeBlockId);
    if (!tab || !tab.connected || !block || block.commands.length === 0) return;
    pendingCommand = '';
    updateAiBlock(activeBlockId, { phase: 'executing' });
    if (liveAgentClient && currentAiSessionId) {
      void liveAgentClient.call('markExecuting', [block.commands]).catch((error) => {
        console.error('Failed to mark live execution state:', error);
      });
    }
    await executeBlockInline(tab, activeBlockId);
  }

  function copyPendingToTerminal() {
    const tab = focusedTab();
    if (!tab) return;
    const block = aiBlocks.find(b => b.id === activeBlockId);
    if (!block) return;
    const cmds = block.commands.map(c => c.text);
    pendingCommand = '';
    removeAiBlock(block.id);
    tab.inputValue = cmds.join(' && ');
    tick().then(() => focusInput());
  }

  function cancelPendingCommand() {
    const tab = focusedTab();
    if (!tab) return;
    pendingCommand = '';
    aiExecPromptShown = false;
    if (liveAgentClient && currentAiSessionId) {
      void liveAgentClient.call('rejectPrompt', ['Execution dismissed by user']).catch((error) => {
        console.error('Failed to reject live prompt:', error);
      });
    }
    // Only remove the active block (choosing/thinking/commands), keep history
    const active = getActiveAiBlock(tab.id);
    if (active) removeAiBlock(active.id);
  }

  function startRename(tabId: string) {
    const tab = tabs.find(t => t.id === tabId);
    if (!tab) return;
    editingTabId = tabId;
    editingTitle = tab.title;
  }

  function commitRename() {
    if (!editingTabId) return;
    const tab = tabs.find(t => t.id === editingTabId);
    if (tab && editingTitle.trim()) {
      tab.title = editingTitle.trim();
      tab.customTitle = true;
    }
    editingTabId = '';
    editingTitle = '';
    focusInput();
  }

  function cancelRename() {
    editingTabId = '';
    editingTitle = '';
    focusInput();
  }

  function autoFocusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function handleTabPointerDown(e: PointerEvent, tabId: string) {
    if (editingTabId === tabId) return;
    pointerStartX = e.clientX;
    isDragging = false;
    dragTabId = tabId;

    const onMove = (ev: PointerEvent) => {
      if (Math.abs(ev.clientX - pointerStartX) > 5) {
        isDragging = true;
      }
      if (!isDragging) return;

      // Find which tab element we're over
      const els = document.querySelectorAll('.tab-item');
      let hoveredId = '';
      els.forEach((el) => {
        const rect = el.getBoundingClientRect();
        if (ev.clientX >= rect.left && ev.clientX <= rect.right) {
          hoveredId = el.getAttribute('data-tab-id') || '';
        }
      });
      dragOverTabId = (hoveredId && hoveredId !== dragTabId) ? hoveredId : '';
    };

    const onUp = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);

      if (isDragging && dragOverTabId && dragTabId !== dragOverTabId) {
        const fromIdx = tabs.findIndex(t => t.id === dragTabId);
        const toIdx = tabs.findIndex(t => t.id === dragOverTabId);
        if (fromIdx !== -1 && toIdx !== -1) {
          const moved = tabs[fromIdx];
          const newTabs = tabs.filter(t => t.id !== dragTabId);
          newTabs.splice(toIdx, 0, moved);
          tabs = newTabs;
        }
      }

      dragTabId = '';
      dragOverTabId = '';
      isDragging = false;
    };

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  function bindTerminalContainer(node: HTMLDivElement) {
    terminalContainerEl = node;
  }

  const runStyleCache = new Map<string, string>();
  function runStyle(style: CellStyle): string {
    // Fast path: no styling at all
    if (!style.fg && !style.bg && !style.bold && !style.dim && !style.italic && !style.underline && !style.inverse && !style.strikethrough) return '';
    // Build a cache key from the style properties
    const key = `${style.fg}|${style.bg}|${+style.bold}${+style.dim}${+style.italic}${+style.underline}${+style.inverse}${+style.strikethrough}`;
    let cached = runStyleCache.get(key);
    if (cached !== undefined) return cached;
    const parts: string[] = [];
    if (style.fg) parts.push(`color:${style.inverse ? style.bg || 'var(--bg-base)' : style.fg}`);
    if (style.bg) parts.push(`background:${style.inverse ? style.fg || 'var(--text-primary)' : style.bg}`);
    if (style.inverse && !style.fg && !style.bg) {
      parts.push('color:var(--bg-base)');
      parts.push('background:var(--text-primary)');
    }
    if (style.bold) parts.push('font-weight:bold');
    if (style.dim) parts.push('opacity:0.6');
    if (style.italic) parts.push('font-style:italic');
    if (style.underline) parts.push('text-decoration:underline');
    if (style.strikethrough) parts.push('text-decoration:line-through');
    cached = parts.join(';');
    runStyleCache.set(key, cached);
    return cached;
  }

  function activeTab(): Tab | undefined {
    return tabs.find(t => t.id === activeTabId);
  }

  function focusedTab(): Tab | undefined {
    if (!paneLayout) return tabs.find(t => t.id === activeTabId);
    const focusedPane = findPane(paneLayout.root, paneLayout.focusedPaneId);
    if (focusedPane?.type === 'leaf') {
      return tabs.find(t => t.id === focusedPane.tabId);
    }
    return tabs.find(t => t.id === activeTabId);
  }

  function getTerminalDimensions(tabId?: string): { rows: number; cols: number } {
    if (tabId && tuiCanvases[tabId]) {
      const container = paneTuiEls[tabId];
      if (container) {
        const rect = container.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          return tuiCanvases[tabId].getDimensions(rect.width, rect.height);
        }
      }
    }
    const el = tabId ? (paneTuiEls[tabId] || paneOutputEls[tabId]) : terminalContainerEl;
    if (!el) return { rows: 24, cols: 80 };
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return { rows: 24, cols: 80 };
    // Subtract horizontal padding (--space-xs = 4px each side on .terminal-line)
    // and scrollbar width (6px) so cols matches what actually fits on screen
    const hPad = 4 * 2 + 6; // left + right padding + scrollbar
    const cols = Math.max(40, Math.floor((rect.width - hPad) / charWidth));
    const rows = Math.max(10, Math.floor(rect.height / charHeight));
    return { rows, cols };
  }

  function handleResize() {
    if (resizeTimer) clearTimeout(resizeTimer);
    resizeTimer = setTimeout(async () => {
      for (const tab of tabs) {
        if (!tab.connected) continue;
        // Resize canvas to fit container if present
        const tc = tuiCanvases[tab.id];
        if (tc && paneTuiEls[tab.id]) {
          tc.fitToContainer(paneTuiEls[tab.id]);
        }
        const { rows, cols } = getTerminalDimensions(tab.id);
        if (rows === tab.emulator.rows && cols === tab.emulator.cols) continue;
        tab.emulator.resize(rows, cols);
        tab.renderVersion++;
        try {
          await invoke('resize_pty', { sessionId: tab.sessionId, rows, cols });
        } catch (e) {
          console.error('Failed to resize PTY:', e);
        }
      }
    }, 100);
  }

  function wireEmulator(tab: Tab) {
    tab.emulator.onTuiModeChange = () => {
      tab.renderVersion++;
      tick().then(() => focusInput());
    };
    tab.emulator.onTitleChange = (title: string) => {
      if (!tab.customTitle) {
        const parts = title.replace(/^(~\/|\/)+/, '').split('/').filter(Boolean);
        tab.title = title === '~' ? '~' : parts.length <= 3 ? title : '…/' + parts.slice(-3).join('/');
      }
    };
    tab.emulator.onCwdChange = (cwdPath: string) => {
      // Normalize: replace home prefix with ~, use forward slashes
      const home = homeDir;
      let cwd = cwdPath.replace(/\\/g, '/').replace(/\/$/, '');
      if (home) {
        const h = home.replace(/\\/g, '/').replace(/\/$/, '');
        if (cwd === h) cwd = '~';
        else if (cwd.startsWith(h + '/')) cwd = '~' + cwd.substring(h.length);
      }
      tab.cwd = cwd;
      tab.cwdFromOsc = true;
      if (!tab.customTitle) {
        const parts = cwd.replace(/^(~\/|\/)+/, '').split('/').filter(Boolean);
        tab.title = cwd === '~' ? '~' : parts.length <= 3 ? cwd : '…/' + parts.slice(-3).join('/');
      }
    };
  }

  async function createTab(): Promise<void> {
    if (creatingTab) return;
    creatingTab = true;
    tabCounter++;
    const tabId = `tab-${tabCounter}`;
    const { rows, cols } = getTerminalDimensions();

    try {
      const sessionId: string = await invoke('spawn_shell', { rows, cols });
      const emu = new TerminalEmulator(rows, cols);
      const newTab = new Tab(tabId, sessionId, emu);
      newTab.title = 'Drover';
      wireEmulator(newTab);
      tabs = [...tabs, newTab];
      activeTabId = tabId;

      if (!paneLayout) {
        paneLayout = createInitialLayout(tabId);
      } else {
        const newRoot = deepCloneNode(paneLayout.root);
        const focusedLeaf = findPane(newRoot, paneLayout.focusedPaneId);
        if (focusedLeaf && focusedLeaf.type === 'leaf') {
          (focusedLeaf as PaneLeaf).tabId = tabId;
        }
        paneLayout = { root: newRoot, focusedPaneId: paneLayout.focusedPaneId };
      }

      await tick();
      focusInput();
      setTimeout(() => updateTabState(newTab), 300);
    } catch (e) {
      console.error('Failed to create tab:', e);
    } finally {
      creatingTab = false;
    }
  }

  async function updateTabCwd(tab: Tab) {
    if (!tab.connected) return;
    // If cwd is already tracked via OSC 7 (shell prompt), skip the backend
    // query which can return stale data on Windows.
    if (tab.cwdFromOsc) return;
    try {
      const cwd: string = await invoke('get_shell_cwd', { sessionId: tab.sessionId });
      if (cwd) {
        tab.cwd = cwd;
        if (!tab.customTitle) {
          const parts = cwd.replace(/^(~\/|\/)+/, '').split('/').filter(Boolean);
          const shortName = cwd === '~' ? '~' : parts.length <= 3 ? cwd : '…/' + parts.slice(-3).join('/');
          tab.title = shortName;
        }
      }
    } catch { /* ignore — CWD lookup can fail briefly after spawn */ }
  }

  async function updateTabSsh(tab: Tab) {
    if (!tab.connected) return;
    try {
      const info: { is_ssh: boolean; target: string; remote_cwd: string; remote_os: string; remote_shell: string } = await invoke('detect_ssh', { sessionId: tab.sessionId });
      tab.sshTarget = info.is_ssh ? info.target : '';
      tab.remoteOs = info.is_ssh ? info.remote_os : '';
      tab.remoteShell = info.is_ssh ? info.remote_shell : '';
      if (info.is_ssh && info.remote_cwd) {
        tab.cwd = info.remote_cwd;
      }
    } catch { /* ignore */ }
  }

  async function updateTabState(tab: Tab) {
    await updateTabSsh(tab);
    if (!tab.sshTarget) {
      await updateTabCwd(tab);
    }
  }

  async function handleSplitPane(paneId: string, direction: 'top' | 'bottom' | 'left' | 'right') {
    if (!paneLayout) return;
    const { rows, cols } = getTerminalDimensions();
    try {
      const sessionId: string = await invoke('spawn_shell', { rows, cols });
      tabCounter++;
      const tabId = `tab-${tabCounter}`;
      const emu = new TerminalEmulator(rows, cols);
      const newTab = new Tab(tabId, sessionId, emu);
      newTab.title = 'Drover';
      wireEmulator(newTab);
      tabs = [...tabs, newTab];
      
      paneLayout = splitPane(paneLayout, paneId, direction, tabId);
      
      requestAnimationFrame(() => focusInput());
      setTimeout(() => updateTabState(newTab), 300);
    } catch (e) {
      console.error('Failed to split pane:', e);
    }
  }

  function handleClosePane(paneId: string) {
    if (!paneLayout) return;
    const pane = findPane(paneLayout.root, paneId);
    const tabId = pane?.type === 'leaf' ? pane.tabId : null;

    const newLayout = closePane(paneLayout, paneId);
    if (newLayout) {
      paneLayout = newLayout;
    } else {
      createTab();
    }

    if (tabId) {
      const tab = tabs.find(t => t.id === tabId);
      if (tab) {
        const wasConnected = tab.connected;
        tab.connected = false;
        if (wasConnected) {
          invoke('write_to_shell', { sessionId: tab.sessionId, data: 'exit\r' }).catch(() => {});
        }
      }
      tabs = tabs.filter(t => t.id !== tabId);
    }
  }

  function handleFocusPane(paneId: string) {
    if (!paneLayout) return;
    paneLayout = { ...paneLayout, focusedPaneId: paneId };
    const pane = findPane(paneLayout.root, paneId);
    if (pane?.type === 'leaf') {
      activeTabId = pane.tabId;
    }
    tick().then(() => focusInput());
  }

  function canClosePane(paneId: string): boolean {
    if (!paneLayout) return false;
    const leaves = getAllLeaves(paneLayout.root);
    return leaves.length > 1;
  }

  // Resize handling for draggable dividers
  let resizingContainer: PaneContainer | null = null;
  let resizingIndex = 0;
  let resizeStartPos = 0;
  let resizeStartSizes: number[] = [];

  function handleResizeStart(e: MouseEvent, index: number, container: PaneContainer) {
    e.preventDefault();
    resizingContainer = container;
    resizingIndex = index;
    resizeStartPos = container.direction === 'horizontal' ? e.clientY : e.clientX;
    resizeStartSizes = [...container.sizes];
    
    document.addEventListener('mousemove', handleResizeMove);
    document.addEventListener('mouseup', handleResizeEnd);
    document.body.style.cursor = container.direction === 'horizontal' ? 'row-resize' : 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleResizeMove(e: MouseEvent) {
    if (!resizingContainer || !paneLayout) return;
    
    const containerEl = document.querySelector(`[data-container-id="${resizingContainer.id}"]`) as HTMLElement;
    if (!containerEl) return;
    
    const isHorizontal = resizingContainer.direction === 'horizontal';
    const containerSize = isHorizontal ? containerEl.offsetHeight : containerEl.offsetWidth;
    const currentPos = isHorizontal ? e.clientY : e.clientX;
    const delta = currentPos - resizeStartPos;
    const deltaPercent = (delta / containerSize) * 100;
    
    const newSizes = [...resizeStartSizes];
    const minSize = 10;
    
    newSizes[resizingIndex] = Math.max(minSize, resizeStartSizes[resizingIndex] + deltaPercent);
    newSizes[resizingIndex + 1] = Math.max(minSize, resizeStartSizes[resizingIndex + 1] - deltaPercent);
    
    const total = newSizes.reduce((a, b) => a + b, 0);
    for (let i = 0; i < newSizes.length; i++) {
      newSizes[i] = (newSizes[i] / total) * 100;
    }
    
    resizingContainer.sizes = newSizes;
    paneLayout = { ...paneLayout };
  }

  function handleResizeEnd() {
    resizingContainer = null;
    document.removeEventListener('mousemove', handleResizeMove);
    document.removeEventListener('mouseup', handleResizeEnd);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }

  let rightPaneWidthPct = $state(50);
  let rpResizing = $state(false);
  let rpResizeStartX = 0;
  let rpResizeStartPct = 0;

  function handleRightPaneResizeStart(e: MouseEvent) {
    e.preventDefault();
    rpResizing = true;
    rpResizeStartX = e.clientX;
    rpResizeStartPct = rightPaneWidthPct;
    document.addEventListener('mousemove', handleRightPaneResizeMove);
    document.addEventListener('mouseup', handleRightPaneResizeEnd);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleRightPaneResizeMove(e: MouseEvent) {
    const splitEl = document.querySelector('.terminal-split') as HTMLElement;
    if (!splitEl) return;
    const totalWidth = splitEl.offsetWidth;
    const dx = rpResizeStartX - e.clientX;
    const dPct = (dx / totalWidth) * 100;
    rightPaneWidthPct = Math.max(15, Math.min(75, rpResizeStartPct + dPct));
  }

  function handleRightPaneResizeEnd() {
    rpResizing = false;
    document.removeEventListener('mousemove', handleRightPaneResizeMove);
    document.removeEventListener('mouseup', handleRightPaneResizeEnd);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }

  // Context menu state
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let contextMenuPaneId = $state('');

  let contextMenuHasSelection = $state(false);
  let contextMenuTabId = $state('');
  let lastTuiPasteEventAt = 0;

  function handleContextMenu(e: MouseEvent, leaf: PaneLeaf) {
    e.preventDefault();
    e.stopPropagation();
    const tc = tuiCanvases[leaf.tabId];
    const browserSel = window.getSelection();
    contextMenuHasSelection = (tc ? tc.hasSelection() : false) || !!(browserSel && browserSel.toString().length > 0);
    contextMenuTabId = leaf.tabId;
    contextMenuX = Math.min(e.clientX, window.innerWidth - 180);
    contextMenuY = Math.min(e.clientY, window.innerHeight - 220);
    contextMenuPaneId = leaf.id;
    showContextMenu = true;
  }

  function contextCopy() {
    const tc = tuiCanvases[contextMenuTabId];
    if (tc && tc.hasSelection()) {
      const text = tc.getSelectedText();
      if (text) navigator.clipboard.writeText(text).catch(() => {});
    } else {
      const sel = window.getSelection();
      if (sel && sel.toString()) {
        const raw = sel.toString();
        const cleaned = raw.split('\n').map(l => l.replace(/\s+$/, '')).join('\n').replace(/\n{3,}/g, '\n\n');
        navigator.clipboard.writeText(cleaned).catch(() => {});
      }
    }
    dismissContextMenu();
  }

  function contextPaste() {
    const tab = tabs.find(t => t.id === contextMenuTabId);
    if (tab && tab.connected) {
      navigator.clipboard.readText().then(text => {
        if (text) invoke('write_to_shell', { sessionId: tab.sessionId, data: text });
      }).catch(() => {});
    }
    dismissContextMenu();
  }

  function dismissContextMenu() {
    showContextMenu = false;
    contextMenuPaneId = '';
  }

  async function contextSplit(direction: 'top' | 'bottom' | 'left' | 'right') {
    const id = contextMenuPaneId;
    dismissContextMenu();
    await handleSplitPane(id, direction);
  }

  function contextClose() {
    const id = contextMenuPaneId;
    dismissContextMenu();
    handleClosePane(id);
  }

  // Drag-to-swap state
  let dragSourcePaneId = $state('');
  let dragTargetPaneId = $state('');

  function handleDragStart(e: DragEvent, leaf: PaneLeaf) {
    dragSourcePaneId = leaf.id;
    e.dataTransfer?.setData('text/plain', leaf.id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }

  function handleDragOver(e: DragEvent, leaf: PaneLeaf) {
    if (!dragSourcePaneId || dragSourcePaneId === leaf.id) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    dragTargetPaneId = leaf.id;
  }

  function handleDragLeave(leaf: PaneLeaf) {
    if (dragTargetPaneId === leaf.id) dragTargetPaneId = '';
  }

  function handleDrop(e: DragEvent, leaf: PaneLeaf) {
    e.preventDefault();
    if (dragSourcePaneId && dragSourcePaneId !== leaf.id && paneLayout) {
      paneLayout = swapPanes(paneLayout, dragSourcePaneId, leaf.id);
    }
    dragSourcePaneId = '';
    dragTargetPaneId = '';
  }

  function handleDragEnd() {
    dragSourcePaneId = '';
    dragTargetPaneId = '';
  }

  function switchTab(tabId: string) {
    const tab = tabs.find(t => t.id === tabId);
    if (!tab) return;
    transition(() => {
      activeTabId = tabId;
      if (paneLayout) {
        const pane = getAllLeaves(paneLayout.root).find(leaf => leaf.tabId === tabId);
        if (pane) {
          paneLayout = { ...paneLayout, focusedPaneId: pane.id };
        } else {
          const newRoot = deepCloneNode(paneLayout.root);
          const focusedLeaf = findPane(newRoot, paneLayout.focusedPaneId);
          if (focusedLeaf && focusedLeaf.type === 'leaf') {
            (focusedLeaf as PaneLeaf).tabId = tabId;
          }
          paneLayout = { root: newRoot, focusedPaneId: paneLayout.focusedPaneId };
        }
      }
    });
    tick().then(() => {
      scrollToBottom();
      focusInput();
    });
  }

  function closeTab(tabId: string) {
    const idx = tabs.findIndex(t => t.id === tabId);
    if (idx === -1) return;
    const tab = tabs[idx];

    // Mark disconnected FIRST to prevent session-ended re-entry
    const wasConnected = tab.connected;
    tab.connected = false;

    if (wasConnected) {
      invoke('write_to_shell', { sessionId: tab.sessionId, data: 'exit\r' }).catch(() => {});
    }
    // Dispose canvas instance if present
    if (tuiCanvases[tabId]) {
      tuiCanvases[tabId].dispose();
      delete tuiCanvases[tabId];
    }

    transition(() => {
      tabs = tabs.filter(t => t.id !== tabId);

      if (tabs.length === 0) {
        paneLayout = null;
        createTab();
        return;
      }

      // Pick new active tab if we closed the current one
      const needSwitch = activeTabId === tabId;
      if (needSwitch) {
        const newIdx = Math.min(idx, tabs.length - 1);
        activeTabId = tabs[newIdx].id;
      }

      // Update pane layout: if pane shows the closed tab, point it to new active tab
      // If pane shows a different tab (multi-pane), remove it from layout
      if (paneLayout) {
        const pane = getAllLeaves(paneLayout.root).find(leaf => leaf.tabId === tabId);
        if (pane) {
          const leaves = getAllLeaves(paneLayout.root);
          if (leaves.length > 1) {
            // Multi-pane: remove the pane showing the closed tab
            const newLayout = closePane(paneLayout, pane.id);
            if (newLayout) paneLayout = newLayout;
          } else {
            // Single pane: just update its tabId to the new active tab
            const newRoot = deepCloneNode(paneLayout.root);
            const leaf = findPane(newRoot, pane.id) as PaneLeaf;
            if (leaf && leaf.type === 'leaf') leaf.tabId = activeTabId;
            paneLayout = { root: newRoot, focusedPaneId: paneLayout.focusedPaneId };
          }
        }
      }
    });
  }

  function scrollIntoAiBlock(el: HTMLElement, active: boolean) {
    if (!active) return;
    requestAnimationFrame(() => {
      el.scrollIntoView({ block: 'end', behavior: 'instant' });
    });
  }

  function autoResizeInput(el: HTMLTextAreaElement) {
    el.style.height = 'auto';
    const h = Math.min(el.scrollHeight, 200);
    el.style.height = h + 'px';
    el.style.overflowY = el.scrollHeight > 200 ? 'auto' : 'hidden';
  }

  function resetInputHeight(tabId: string) {
    const el = paneInputEls[tabId];
    if (el) { el.style.height = ''; el.style.overflowY = 'hidden'; }
  }

  async function sendCommand() {
    const tab = focusedTab();
    if (!tab || !tab.inputValue.trim() || !tab.connected) return;

    const cmd = tab.inputValue.trim();
    tab.inputValue = '';
    resetInputHeight(tab.id);
    tab.historyIndex = -1;

    tab.commandHistory = [cmd, ...tab.commandHistory];
    autocompleteSuggestion = '';

    // Add to global history (deduplicated, most recent first)
    globalHistory = [cmd, ...globalHistory.filter(h => h !== cmd)].slice(0, 500);
    saveGlobalHistory();

    tab.awaitingInput = false;
    tab.commandRunning = true;

    try {
      await invoke('write_to_shell', { sessionId: tab.sessionId, data: cmd + '\r' });
    } catch (e) {
      console.error('Failed to write to shell:', e);
    }

    await tick();
    scrollToBottom();

    // Update tab title and SSH state after command
    setTimeout(() => updateTabState(tab), 500);
  }

  function scrollToBottom(tabId?: string) {
    const id = tabId || (paneLayout ? (findPane(paneLayout.root, paneLayout.focusedPaneId) as PaneLeaf)?.tabId : activeTabId) || activeTabId;
    tick().then(() => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const el = paneOutputEls[id];
          if (el) el.scrollTop = el.scrollHeight;
        });
      });
    });
  }

  function handleKeydown(e: KeyboardEvent) {
    const tab = focusedTab();
    if (!tab) return;

    // Handle pending AI command confirmation (manual mode only, not during exec mode prompt)
    if (pendingCommand && !aiExecPromptShown) {
      if (e.key === 'Enter') {
        e.preventDefault();
        executePendingCommand();
        return;
      } else if (e.key === 'Escape') {
        e.preventDefault();
        cancelPendingCommand();
        return;
      }
    }
    // Allow Escape to dismiss exec mode prompt
    if (aiExecPromptShown && e.key === 'Escape') {
      e.preventDefault();
      cancelPendingCommand();
      return;
    }

    // When a command is running, forward keystrokes directly to PTY
    // so interactive prompts (arrow-key menus, yes/no, etc.) work
    if (tab.commandRunning && tab.connected) {
      // Keep app shortcuts working while the PTY owns the input stream.
      if (isPrimaryModifier(e)) {
        if (e.key === 't') { e.preventDefault(); createTab(); return; }
        if (e.key === 'w') { e.preventDefault(); closeTab(tab.id); return; }
        if (e.key === 'k' || e.key === 'l') { e.preventDefault(); tab.emulator.reset(); tab.renderVersion++; return; }
        if (e.key === 'c') return; // let browser handle copy
        if (e.key >= '1' && e.key <= '9') { e.preventDefault(); const idx = parseInt(e.key) - 1; if (idx < tabs.length) switchTab(tabs[idx].id); return; }
      }
      // Ctrl+C sends SIGINT and also stops commandRunning
      if (e.key === 'c' && e.ctrlKey && !e.metaKey) {
        const sel = window.getSelection();
        if (sel && sel.toString().length > 0) return;
        e.preventDefault();
        invoke('write_to_shell', { sessionId: tab.sessionId, data: '\x03' });
        tab.commandRunning = false;
        tab.awaitingInput = false;
        return;
      }
      const seq = keyToSequence(e);
      if (seq) {
        e.preventDefault();
        invoke('write_to_shell', { sessionId: tab.sessionId, data: seq });
      }
      return;
    }

    // Slash command menu navigation
    if (slashMenuOpen) {
      const filtered = getFilteredSlashCommands();
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        slashMenuIndex = Math.min(slashMenuIndex + 1, filtered.length - 1);
        return;
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        slashMenuIndex = Math.max(slashMenuIndex - 1, 0);
        return;
      } else if ((e.key === 'Enter' || e.key === 'Tab') && filtered.length > 0) {
        e.preventDefault();
        selectSlashCommand(filtered[slashMenuIndex]);
        return;
      } else if (e.key === 'Escape') {
        e.preventDefault();
        closeSlashMenu();
        return;
      }
    }

    // @ mention file picker navigation
    if (atMenuOpen) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        atMenuIndex = Math.min(atMenuIndex + 1, atFileSuggestions.length - 1);
        return;
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        atMenuIndex = Math.max(atMenuIndex - 1, 0);
        return;
      } else if ((e.key === 'Enter' || e.key === 'Tab') && atFileSuggestions.length > 0) {
        e.preventDefault();
        selectAtFile(atFileSuggestions[atMenuIndex]);
        return;
      } else if (e.key === 'Escape') {
        e.preventDefault();
        closeAtMenu();
        return;
      }
    }

    if (e.key === 'Tab' && autocompleteSuggestion && !agenticMode) {
      e.preventDefault();
      acceptAutocomplete();
      return;
    }

    if (e.key === 'Escape') {
      autocompleteSuggestion = '';
      return;
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (agenticMode) {
        handleAgenticSubmit();
      } else if (!tab.inputValue.trim() && tab.connected) {
        // Empty input: forward Enter directly to PTY so interactive programs
        // (e.g. claude, codex trust dialogs) receive it
        invoke('write_to_shell', { sessionId: tab.sessionId, data: '\r' });
      } else {
        sendCommand();
      }
    } else if (e.key === 'ArrowUp') {
      if (tab.commandHistory.length > 0 && tab.historyIndex < tab.commandHistory.length - 1) {
        tab.historyIndex++;
        tab.inputValue = tab.commandHistory[tab.historyIndex];
        e.preventDefault();
        tick().then(() => { const el = paneInputEls[tab.id]; if (el) autoResizeInput(el); });
      }
    } else if (e.key === 'ArrowDown') {
      if (tab.historyIndex > 0) {
        tab.historyIndex--;
        tab.inputValue = tab.commandHistory[tab.historyIndex];
      } else {
        tab.historyIndex = -1;
        tab.inputValue = '';
      }
      e.preventDefault();
      tick().then(() => { const el = paneInputEls[tab.id]; if (el) autoResizeInput(el); });
    } else if (e.key === 'c' && e.ctrlKey && !e.metaKey) {
      // Ctrl+C: copy if text selected, otherwise send SIGINT
      const sel = window.getSelection();
      if (sel && sel.toString().length > 0) {
        // Let the browser handle copy
        return;
      }
      e.preventDefault();
      if (tab.connected) {
        invoke('write_to_shell', { sessionId: tab.sessionId, data: '\x03' });
      }
    } else if (e.key === 'c' && isPrimaryModifier(e)) {
      // Let the browser handle primary-modifier copy for selected text.
      return;
    } else if (e.key === 'd' && e.ctrlKey && !e.metaKey) {
      // Ctrl+D: send EOF
      e.preventDefault();
      if (tab.connected) {
        invoke('write_to_shell', { sessionId: tab.sessionId, data: '\x04' });
      }
    } else if (e.key === 'k' && isPrimaryModifier(e)) {
      // Primary+K: clear terminal
      e.preventDefault();
      tab.emulator.reset();
      tab.renderVersion++;
      scrollToBottom();
    } else if (e.key === 'l' && isPrimaryModifier(e)) {
      e.preventDefault();
      tab.emulator.reset();
      tab.renderVersion++;
    } else if (e.key === 't' && isPrimaryModifier(e)) {
      e.preventDefault();
      createTab();
    } else if (e.key === 'w' && isPrimaryModifier(e)) {
      e.preventDefault();
      if (paneLayout && getAllLeaves(paneLayout.root).length > 1) {
        // Close current pane if multiple panes exist
        handleClosePane(paneLayout.focusedPaneId);
      } else {
        // Close tab if only one pane
        closeTab(activeTabId);
      }
    } else if (e.key === 'ArrowUp' && isPrimaryModifier(e) && e.shiftKey) {
      e.preventDefault();
      if (paneLayout) handleSplitPane(paneLayout.focusedPaneId, 'top');
    } else if (e.key === 'ArrowDown' && isPrimaryModifier(e) && e.shiftKey) {
      e.preventDefault();
      if (paneLayout) handleSplitPane(paneLayout.focusedPaneId, 'bottom');
    } else if (e.key === 'ArrowLeft' && isPrimaryModifier(e) && e.shiftKey) {
      e.preventDefault();
      if (paneLayout) handleSplitPane(paneLayout.focusedPaneId, 'left');
    } else if (e.key === 'ArrowRight' && isPrimaryModifier(e) && e.shiftKey) {
      e.preventDefault();
      if (paneLayout) handleSplitPane(paneLayout.focusedPaneId, 'right');
    } else if (isPrimaryModifier(e) && e.key >= '1' && e.key <= '9') {
      // Primary+1..9: switch to tab by index
      e.preventDefault();
      const idx = parseInt(e.key) - 1;
      if (idx < tabs.length) {
        switchTab(tabs[idx].id);
      }
    }
  }

  function handleTuiCopy(e: ClipboardEvent, tab: Tab) {
    const tc = tuiCanvases[tab.id];
    if (!tc || !tc.hasSelection()) return;
    const text = tc.getSelectedText();
    if (text) {
      e.preventDefault();
      e.clipboardData?.setData('text/plain', text);
    }
  }

  function handleTuiPaste(e: ClipboardEvent, tab: Tab) {
    if (!tab.connected) return;
    const text = e.clipboardData?.getData('text/plain') ?? '';
    if (!text) return;
    e.preventDefault();
    lastTuiPasteEventAt = performance.now();
    invoke('write_to_shell', { sessionId: tab.sessionId, data: text });
  }

  function handleTuiWheel(e: WheelEvent, tab: Tab) {
    if (!tab.connected) return;
    e.preventDefault();
    const lines = Math.max(1, Math.round(Math.abs(e.deltaY) / 20));
    const emu = tab.emulator;
    const tc = tuiCanvases[tab.id];
    const scrollingUp = e.deltaY < 0;

    if (emu.mouseTrackingMode !== 'none') {
      const cell = tc ? tc.pixelToCell(e.clientX, e.clientY) : { row: 0, col: 0 };
      const col = cell.col;
      const row = cell.row;
      const button = scrollingUp ? 64 : 65;
      for (let i = 0; i < lines; i++) {
        const seq = encodeMouseEvent(emu, button, col, row, true);
        if (seq) invoke('write_to_shell', { sessionId: tab.sessionId, data: seq });
      }
    } else {
      const arrow = scrollingUp ? '\x1b[A' : '\x1b[B';
      for (let i = 0; i < lines; i++) {
        invoke('write_to_shell', { sessionId: tab.sessionId, data: arrow });
      }
    }

    if (tc) {
      tc.handleBufferScroll(!scrollingUp, lines);
    }
  }

  function handleTuiKeydown(e: KeyboardEvent, tab: Tab) {
    if (!tab.connected) return;
    // App shortcuts still work in TUI mode.
    if (e.key === 't' && isPrimaryModifier(e)) {
      e.preventDefault();
      createTab();
      return;
    }
    if (e.key === 'w' && isPrimaryModifier(e)) {
      e.preventDefault();
      closeTab(tab.id);
      return;
    }
    if (e.key === 'c' && isPrimaryModifier(e)) {
      // Primary+C copies selected text from the canvas if present.
      const tc = tuiCanvases[tab.id];
      if (!tc || !tc.hasSelection()) return;
      e.preventDefault();
      document.execCommand('copy');
      return;
    }
    if (e.key === 'v' && isPrimaryModifier(e)) {
      e.preventDefault();
      navigator.clipboard.readText().then(text => {
        if (text && tab.connected) {
          invoke('write_to_shell', { sessionId: tab.sessionId, data: text });
        }
      }).catch(() => {});
      return;
    }
    if (isPrimaryModifier(e) && e.key >= '1' && e.key <= '9') {
      e.preventDefault();
      const idx = parseInt(e.key) - 1;
      if (idx < tabs.length) switchTab(tabs[idx].id);
      return;
    }
    // Send raw keystroke to PTY
    const seq = keyToSequence(e);
    if (seq) {
      e.preventDefault();
      invoke('write_to_shell', { sessionId: tab.sessionId, data: seq });
    }
  }

  function initTuiCanvas(canvasEl: HTMLCanvasElement, tab: Tab) {
    const tc = new TerminalCanvas(canvasEl, tab.emulator, {
      fontFamily: "'JetBrains Mono', 'Fira Code', 'SF Mono', 'Cascadia Code', 'Menlo', monospace",
      fontSize: 13,
      lineHeight: 1.2,
      defaultFg: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() || '#e4e4ed',
      defaultBg: getComputedStyle(document.documentElement).getPropertyValue('--bg-base').trim() || '#0a0a0f',
      cursorColor: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() || '#e4e4ed',
      selectionColor: 'rgba(68, 138, 255, 0.35)',
      padding: [4, 8, 4, 8],
    });
    tuiCanvases[tab.id] = tc;

    // Forward mouse events to PTY for TUI apps with mouse tracking
    tc.onMouseInput = (button, col, row, isRelease) => {
      if (!tab.connected) return;
      const seq = encodeMouseEvent(tab.emulator, button, col, row, isRelease);
      if (seq) invoke('write_to_shell', { sessionId: tab.sessionId, data: seq });
    };

    // Size canvas to container after mount
    requestAnimationFrame(() => {
      const container = paneTuiEls[tab.id];
      if (container) {
        tc.fitToContainer(container);
        const { rows, cols } = tc.getDimensions(container.getBoundingClientRect().width, container.getBoundingClientRect().height);
        if (rows !== tab.emulator.rows || cols !== tab.emulator.cols) {
          tab.emulator.resize(rows, cols);
          tab.renderVersion++;
          invoke('resize_pty', { sessionId: tab.sessionId, rows, cols }).catch(() => {});
        }
      }
    });

    return {
      destroy() {
        tc.dispose();
        delete tuiCanvases[tab.id];
      }
    };
  }

  function focusInput() {
    const selection = window.getSelection();
    if (selection && selection.toString().length > 0) return;
    const id = paneLayout ? (findPane(paneLayout.root, paneLayout.focusedPaneId) as PaneLeaf)?.tabId || activeTabId : activeTabId;
    const tab = tabs.find(t => t.id === id);
    if (tab && tab.isTui) {
      paneTuiEls[id]?.focus();
    } else {
      paneInputEls[id]?.focus();
    }
  }

  onMount(() => {
    let resizeObserver: ResizeObserver;

    desktopPlatform = detectDesktopPlatform();
    document.documentElement.dataset.platform = desktopPlatform;
    getHomeDir().then(h => { homeDir = h; }).catch(() => {});

    async function init() {
      measureCharMetrics();

      // Register global listeners ONCE
      unlistenOutput = await listen<{ session_id: string; data: string }>('terminal-output', (event) => {
        const tab = tabs.find(t => t.sessionId === event.payload.session_id);
        if (!tab) return;
        tab.emulator.write(event.payload.data);
        // Detect shell prompt return to clear commandRunning
        if (tab.commandRunning && !tab.isTui) {
          const lines = tab.emulator.getLines();
          const curRow = tab.emulator.cursorRow;
          if (curRow >= 0 && curRow < lines.length) {
            const lineText = lines[curRow].map((r: { text: string }) => r.text).join('').trimEnd();
            // Only clear commandRunning when we see a bare prompt (no command text after symbol)
            // This avoids false positives from echoed command lines like "PS C:\path> claude"
            if (lineText && isBarePrompt(lineText)) {
              tab.commandRunning = false;
              tab.awaitingInput = false;
            } else {
              tab.awaitingInput = detectAwaitingInput(lines, curRow);
            }
          }
        } else if (!tab.commandRunning) {
          tab.awaitingInput = false;
        }
        dirtyTabIds.add(tab.id);
        if (!rafPending) {
          rafPending = true;
          requestAnimationFrame(() => {
            for (const id of dirtyTabIds) {
              const t = tabs.find(t => t.id === id);
              if (t) t.renderVersion++;
              // Mark canvas dirty for TUI tabs
              if (tuiCanvases[id]) tuiCanvases[id].markDirty();
              // Scroll visible panes
              if (id === activeTabId) scrollToBottom(id);
              else if (paneLayout) {
                const leaves = getAllLeaves(paneLayout.root);
                if (leaves.some(leaf => leaf.tabId === id)) scrollToBottom(id);
              }
            }
            dirtyTabIds.clear();
            rafPending = false;
          });
        }
      });

      unlistenEnded = await listen<string>('session-ended', (event) => {
        const tab = tabs.find(t => t.sessionId === event.payload);
        if (!tab || !tab.connected) return;
        tab.connected = false;
        closeTab(tab.id);
      });

      await createTab();
      focusInput();

      // Load settings in background — don't block terminal creation
      loadSettings();

      resizeObserver = new ResizeObserver(() => handleResize());
      const splitEl = document.querySelector('.terminal-split');
      if (splitEl) {
        resizeObserver.observe(splitEl);
      } else if (terminalContainerEl) {
        resizeObserver.observe(terminalContainerEl);
      }
    }

    init();

    return () => {
      resizeObserver?.disconnect();
      unlistenOutput?.();
      unlistenEnded?.();
      liveAgentClient?.close();
    };
  });
</script>

<svelte:window onclick={() => { toolsDropdownOpen = false; sshDropdownOpen = false; if (slashMenuOpen) closeSlashMenu(); if (atMenuOpen) closeAtMenu(); if (terminalFileCtxMenu) closeTerminalFileCtxMenu(); if (currentView === 'terminal') focusInput(); }} onkeydown={handleGlobalKeydown} onpaste={handlePaste} />

<div class="app-layout">
  {#if fileExplorerOpen}
    <FileExplorer
      bind:root={fileExplorerRoot}
      bind:open={fileExplorerOpen}
      bind:showHidden={showHiddenFiles}
      sshTarget={focusedTab()?.sshTarget ?? ''}
      agentMode={agenticMode}
      onOpenFile={openFile}
      onPreviewFile={previewFile}
      onNavigate={handleExplorerNavigate}
      onAddToChat={(entry) => addFileToChat(entry)}
      getInitialCwd={getFileExplorerCwd}
    />
  {/if}
  {#if aiSessionsSidebarOpen}
    <aside style="width: 320px; min-width: 320px; max-width: 320px; border-right: 1px solid var(--border-subtle); background: color-mix(in srgb, var(--bg-elevated) 88%, black); display: flex; flex-direction: column;">
      <div style="padding: 14px 14px 10px; border-bottom: 1px solid var(--border-subtle); display: flex; align-items: center; justify-content: space-between; gap: 10px;">
        <div>
          <div style="font-size: 12px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted);">AI Sessions</div>
          <div style="font-size: 13px; color: var(--text-secondary); margin-top: 4px;">Resume, search, and review prior runs</div>
        </div>
        <button class="toolbar-icon-btn" onclick={() => aiSessionsSidebarOpen = false} title="Close sessions" aria-label="Close sessions">
          <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
      <div style="padding: 12px 14px; border-bottom: 1px solid var(--border-subtle); display: flex; flex-direction: column; gap: 10px;">
        <button class="ai-action-btn ai-action-btn-primary" style="width: 100%; justify-content: center;" onclick={startNewAiSession}>
          <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          New session
        </button>
        <input
          type="text"
          bind:value={aiSessionSearch}
          placeholder="Search sessions"
          spellcheck="false"
          style="width: 100%; border: 1px solid var(--border-subtle); background: var(--bg-base); color: var(--text-primary); border-radius: 10px; padding: 10px 12px; font: inherit;"
        />
      </div>
      <div style="flex: 1; overflow: auto; padding: 10px;">
        {#if aiSessionActionError}
          <div style="padding: 12px; color: var(--term-red); white-space: pre-wrap; border: 1px solid color-mix(in srgb, var(--term-red) 35%, transparent); border-radius: 12px; margin-bottom: 10px; background: color-mix(in srgb, var(--term-red) 10%, var(--bg-base));">{aiSessionActionError}</div>
        {/if}
        {#if aiSessionsLoading}
          <div style="padding: 12px; color: var(--text-muted);">Loading sessions...</div>
        {:else if aiSessionsError}
          <div style="padding: 12px; color: var(--term-red); white-space: pre-wrap;">{aiSessionsError}</div>
        {:else if getFilteredAiSessions().length === 0}
          <div style="padding: 12px; color: var(--text-muted);">{aiSessionSearch ? 'No sessions match your search.' : 'No sessions yet.'}</div>
        {:else}
          {#each getFilteredAiSessions() as session}
            <button
              onclick={() => resumeAiSession(session)}
              style="width: 100%; text-align: left; padding: 12px; border: 1px solid color-mix(in srgb, var(--border-subtle) 90%, transparent); background: {currentAiSessionId === session.id ? 'color-mix(in srgb, var(--accent-primary) 14%, var(--bg-base))' : 'var(--bg-base)'}; color: var(--text-primary); border-radius: 12px; display: flex; flex-direction: column; gap: 6px; margin-bottom: 10px;"
            >
              <div style="display: flex; align-items: center; justify-content: space-between; gap: 10px;">
                <span style="font-weight: 600; font-size: 13px;">{session.title || session.lastUserMessage || 'Untitled session'}</span>
                <span style="font-size: 11px; color: var(--text-muted); white-space: nowrap;">{session.updated_at}</span>
              </div>
              {#if session.lastUserMessage}
                <div style="font-size: 12px; color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">{session.lastUserMessage}</div>
              {/if}
              <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 11px; color: var(--text-muted);">
                <span>{session.model}</span>
                <span>{session.messageCount} turns</span>
              </div>
            </button>
          {/each}
        {/if}
      </div>
    </aside>
  {/if}
  <div class="main-area">
    <!-- Titlebar -->
    <header class="titlebar" data-tauri-drag-region>
      {#if currentView === 'settings'}
        <div class="titlebar-settings-nav">
          <button class="titlebar-back" onclick={closeSettings}>Back to Terminal</button>
          <span class="titlebar-title" data-tauri-drag-region>Settings</span>
        </div>
      {:else}
        <div class="tab-bar">
          {#each tabs as tab}
            <div
              class="tab-item"
              class:active={tab.id === activeTabId}
              style={tab.id === activeTabId ? 'view-transition-name: active-tab' : ''}
              class:drag-over={dragOverTabId === tab.id}              class:dragging={dragTabId === tab.id && isDragging}
              data-tab-id={tab.id}
              onclick={() => { if (!isDragging) switchTab(tab.id); }}
              ondblclick={() => startRename(tab.id)}
              onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') switchTab(tab.id); }}
              onpointerdown={(e: PointerEvent) => handleTabPointerDown(e, tab.id)}
              role="tab"
              tabindex="0"
            >
              {#if editingTabId === tab.id}
                <input
                  class="tab-rename-input"
                  type="text"
                  bind:value={editingTitle}
                  onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') cancelRename(); e.stopPropagation(); }}
                  onblur={commitRename}
                  onclick={(e: MouseEvent) => e.stopPropagation()}
                  use:autoFocusSelect
                />
              {:else}
                <span class="tab-title">{tab.title}</span>
              {/if}
              {#if tabs.length > 1 && editingTabId !== tab.id}
                <span
                  class="tab-close"
                  role="button"
                  tabindex="0"
                  onclick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
                  onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); closeTab(tab.id); } }}
                  aria-label="Close tab"
                >&times;</span>
              {/if}
            </div>
          {/each}
          <button class="tab-add" onclick={() => createTab()} aria-label="New tab">+</button>
        </div>
      {/if}
      <div class="titlebar-spacer" data-tauri-drag-region></div>
      <div class="titlebar-caption-space" data-tauri-drag-region aria-hidden="true"></div>
    </header>

    {#if currentView === 'settings'}
      <!-- Settings Page -->
      <div class="settings-page">
        <SettingsPage
          bind:aiAccountId
          bind:aiApiToken
          bind:geminiApiKey
          bind:activeThemeId
          bind:customTheme
          sshSessions={sshSessions}
          savedCommands={savedCommands}
          mcpServers={mcpServers}
          onSaveSettings={saveSettings}
          onRevokeCredentials={revokeCredentials}
          onSaveGeminiKey={saveGeminiKey}
          onRevokeGeminiKey={revokeGeminiKey}
          onSaveTheme={saveThemeToStore}
          onAddSshSession={addSshSession}
          onRemoveSshSession={removeSshSession}
          onAddSavedCommand={addSavedCommand}
          onRemoveSavedCommand={removeSavedCommand}
          onAddMcpServer={addMcpServer}
          onRemoveMcpServer={removeMcpServer}
        />
      </div>
    {:else}
      <!-- Terminal Area -->
      <div class="terminal-split">
        {#if paneLayout}
          {#snippet renderPane(node: PaneNode)}
            {#if node.type === 'leaf'}
              {@const paneLeaf = node as PaneLeaf}
              {#each tabs.filter(t => t.id === paneLeaf.tabId).slice(0, 1) as tab (tab.id)}
              <div class="terminal-pane" class:focused={paneLayout!.focusedPaneId === paneLeaf.id}
                oncontextmenu={(e) => handleContextMenu(e, paneLeaf)}
                class:drag-target={dragTargetPaneId === paneLeaf.id}
                ondragover={(e) => handleDragOver(e, paneLeaf)}
                ondragleave={() => handleDragLeave(paneLeaf)}
                ondrop={(e) => handleDrop(e, paneLeaf)}
              >
                <div class="pane-header"
                  draggable="true"
                  ondragstart={(e) => handleDragStart(e, paneLeaf)}
                  ondragend={handleDragEnd}
                >
                  <span class="pane-header-drag" aria-hidden="true">
                    <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor"><circle cx="2" cy="2" r="1.2"/><circle cx="8" cy="2" r="1.2"/><circle cx="2" cy="7" r="1.2"/><circle cx="8" cy="7" r="1.2"/><circle cx="2" cy="12" r="1.2"/><circle cx="8" cy="12" r="1.2"/></svg>
                  </span>
                  <span class="pane-header-title">{tab.title}</span>
                  {#if paneLayout && getAllLeaves(paneLayout.root).length > 1}
                    <button class="pane-header-close" onclick={(e) => { e.stopPropagation(); handleClosePane(paneLeaf.id); }} title="Close pane" aria-label="Close pane">×</button>
                  {/if}
                </div>
              {#if tab.isTui}
                <!-- TUI Mode: canvas-based rendering -->
                <div
                  class="terminal-container tui-mode"
                  bind:this={paneTuiEls[paneLeaf.tabId]}
                  use:bindTerminalContainer
                  onclick={() => {
                    if (paneLayout) {
                      const pane = getAllLeaves(paneLayout.root).find(leaf => leaf.tabId === paneLeaf.tabId);
                      if (pane) handleFocusPane(pane.id);
                    }
                  }}
                  onkeydown={(e) => handleTuiKeydown(e, tab)}
                  onwheel={(e) => handleTuiWheel(e, tab)}
                  oncopy={(e) => handleTuiCopy(e, tab)}
                  onpaste={(e) => handleTuiPaste(e, tab)}
                  role="textbox"
                  tabindex="0"
                >
                  <canvas
                    class="terminal-canvas"
                    use:initTuiCanvas={tab}
                  ></canvas>
                </div>
              {:else}
                <!-- Normal Mode: scrollback + input bar -->
                <div class="terminal-container" bind:this={terminalContainerEl} onclick={() => {
                  if (paneLayout) {
                    const pane = getAllLeaves(paneLayout.root).find(leaf => leaf.tabId === paneLeaf.tabId);
                    if (pane) handleFocusPane(pane.id);
                  }
                  focusInput();
                }} role="presentation">
                  <div class="terminal-output" bind:this={paneOutputEls[paneLeaf.tabId]} onclick={(e) => handleTerminalFileClick(e, tab)} oncontextmenu={(e) => handleTerminalFileContextMenu(e, tab)} onmousemove={handleTerminalMouseMove} onmouseleave={handleTerminalMouseLeave}>
                    {#if tab.renderedLines.length === 0 || (tab.renderedLines.length <= tab.emulator.rows && tab.renderedLines.every((r: { text: string; style: CellStyle }[]) => r.length === 0 || (r.length === 1 && r[0].text === '')))}
                      <div class="welcome-block">
                        <div class="welcome-logo">Drover</div>
                        <p class="welcome-subtitle">
                          A modern terminal built with Rust.
                        </p>
                      </div>
                    {/if}

                    {#each tab.renderedLines as row, i}
                      {@const text = row.map(r => r.text).join('')}
                      {@const hasAnsi = row.some(r => r.style.fg || r.style.bg || r.style.bold || r.style.italic || r.style.underline)}
                      {@const prompt = !hasAnsi ? highlightPrompt(text) : null}
                      {@const semantic = !hasAnsi && !prompt ? highlightOutput(text) : null}
                      {#if text.trim()}
                        <div class="terminal-line" class:terminal-prompt-line={prompt} class:terminal-prompt-line-last={prompt && i === getLastPromptIndex(tab.renderedLines)}>
                          {#if prompt}
                            {#each prompt as seg}
                              <span class="terminal-run" style={seg.color ? `color:${seg.color}` : ''}>{seg.text}</span>
                            {/each}
                          {:else if semantic}
                            {#each semantic as seg}
                              <span class="terminal-run" style={seg.color ? `color:${seg.color}` : ''}>{seg.text}</span>
                            {/each}
                          {:else}
                            {#each row as run}
                              {#if run.text}
                                <span class="terminal-run" style={runStyle(run.style)}>{run.text}</span>
                              {/if}
                            {/each}
                          {/if}
                        </div>
                      {:else}
                        <div class="terminal-line">&nbsp;</div>
                      {/if}
                    {/each}

                    <!-- AI blocks (history + active) -->
                    {#each getVisibleAiBlocks(paneLeaf.tabId) as block (block.id)}
                      {#if block.phase === 'passthrough'}
                        <!-- Inline passthrough command -->
                        <div class="ai-thread ai-thread-passthrough">
                          <div class="ai-passthrough-block">
                            <div class="ai-passthrough-cmd">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
                              <code>{block.prompt}</code>
                              {#if block.commands[0]?.status === 'running'}
                                <span class="ai-spinner-sm"></span>
                              {/if}
                              <button class="ai-block-dismiss" onclick={() => removeAiBlock(block.id)} title="Remove">
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                              </button>
                            </div>
                            {#if block.commands[0]?.output}
                              <pre class="ai-passthrough-output">{block.commands[0].output}</pre>
                            {/if}
                          </div>
                        </div>
                      {:else}
                      <div class="ai-thread" use:scrollIntoAiBlock={block.phase !== 'done' && block.phase !== 'error' && block.phase !== 'answered'}>
                        <!-- User question -->
                        <div class="ai-thread-question">
                          <div class="ai-thread-label">
                            <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path><circle cx="12" cy="7" r="4"></circle></svg>
                            You
                          </div>
                          <div class="ai-thread-question-text">{block.prompt}</div>
                          {#if block.attachments && block.attachments.length > 0}
                            <div class="ai-thread-attachments">
                              {#each block.attachments as attachment}
                                <button 
                                  class="ai-thread-attachment-chip" 
                                  onclick={() => { 
                                    if (attachment.mimeType.startsWith('image/')) {
                                      previewUrl = `data:${attachment.mimeType};base64,${attachment.content}`;
                                      previewFileName = attachment.name;
                                      rightPane = 'preview';
                                    }
                                  }}
                                  title={attachment.name}
                                >
                                  {#if attachment.mimeType.startsWith('image/')}
                                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>
                                  {:else}
                                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>
                                  {/if}
                                  <span class="ai-thread-attachment-name">{attachment.name}</span>
                                </button>
                              {/each}
                            </div>
                          {/if}
                          {#if block.phase === 'done' || block.phase === 'error' || block.phase === 'answered' || block.phase === 'needs_input'}
                            <button class="ai-block-dismiss" onclick={() => removeAiBlock(block.id)} title="Remove">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                            </button>
                          {:else if block.phase !== 'choosing'}
                            <button class="ai-block-dismiss" onclick={cancelPendingCommand} title="Cancel">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                            </button>
                          {/if}
                        </div>

                        {#if block.phase === 'choosing'}
                          <!-- Execution mode chooser -->
                          <div class="ai-thread-answer">
                            <div class="ai-thread-label ai-thread-label-ai">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path></svg>
                              Drover
                            </div>
                            <div class="ai-block-question">How should I execute commands?</div>
                            <div class="ai-block-actions ai-block-actions-end">
                              <div class="ai-exec-dropdown-wrap">
                                <button class="ai-action-btn ai-action-btn-primary" onclick={() => chooseAiExecMode('auto')}>
                                  <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                                  Auto-execute
                                </button>
                                <button class="ai-exec-dropdown-toggle" aria-label="More options" onclick={() => { aiExecDropdownOpen = !aiExecDropdownOpen; }}>
                                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><polyline points="6 9 12 15 18 9"></polyline></svg>
                                </button>
                                {#if aiExecDropdownOpen}
                                  <div class="ai-exec-dropdown-menu">
                                    <button class="ai-exec-dropdown-item" onclick={() => { aiExecDropdownOpen = false; chooseAiExecMode('manual'); }}>
                                      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><rect x="3" y="3" width="18" height="18" rx="2"></rect><path d="M9 12l2 2 4-4"></path></svg>
                                      Review first
                                    </button>
                                  </div>
                                {/if}
                              </div>
                            </div>
                          </div>

                        {:else if block.phase === 'thinking'}
                          <div class="ai-thread-status">
                            <span class="ai-spinner"></span> Thinking...
                          </div>

                        {:else if block.phase === 'error'}
                          <div class="ai-thread-answer ai-thread-answer-error">
                            <div class="ai-thread-label ai-thread-label-ai">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path></svg>
                              Drover
                            </div>
                            <div class="ai-thread-error-text">{block.error}</div>
                          </div>

                        {:else}
                          <!-- Commands section (shown during executing/retrying/commands/done/answered) -->
                          {#if block.commands.length > 0 || block.toolCalls.length > 0}
                            <div class="ai-thread-commands" class:ai-thread-commands-collapsed={block.phase === 'answered'}>
                              <button class="ai-thread-commands-header" onclick={(e) => { const el = (e.currentTarget as HTMLElement).parentElement; el?.classList.toggle('ai-thread-commands-collapsed'); }}>
                                <svg class="ai-thread-commands-chevron" viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><polyline points="6 9 12 15 18 9"></polyline></svg>
                                <span class="ai-thread-commands-label">
                                  {#if block.phase === 'executing' || block.phase === 'retrying'}
                                    <span class="ai-spinner-sm"></span>
                                  {:else}
                                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
                                  {/if}
                                  {#if block.phase === 'retrying'}
                                    Retrying with alternative...
                                  {:else if block.phase === 'executing'}
                                    Running {getAiExecutedCount(block)} step{getAiExecutedCount(block) > 1 ? 's' : ''}...
                                  {:else if block.phase === 'needs_input'}
                                    Waiting for input
                                  {:else}
                                    {getAiExecutedCount(block)} step{getAiExecutedCount(block) > 1 ? 's' : ''} executed
                                  {/if}
                                </span>
                              </button>
                              <div class="ai-thread-commands-body">
                                {#if block.toolCalls.length > 0}
                                  <div class="ai-thread-section-heading">MCP tool calls</div>
                                  <div class="ai-block-cmds">
                                    {#each block.toolCalls as tc, ti}
                                      <div class="ai-cmd-entry">
                                        <div class="ai-cmd-row" class:ai-cmd-running={tc.status === 'running'} class:ai-cmd-done={tc.status === 'done'} class:ai-cmd-error={tc.status === 'error'}>
                                          <span class="ai-cmd-indicator">
                                            {#if tc.status === 'running'}
                                              <span class="ai-spinner-sm"></span>
                                            {:else if tc.status === 'done'}
                                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="var(--term-green)" stroke-width="2.5" fill="none"><polyline points="20 6 9 17 4 12"></polyline></svg>
                                            {:else if tc.status === 'error'}
                                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="var(--term-red)" stroke-width="2.5" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                                            {:else}
                                              <span class="ai-cmd-num">{ti + 1}</span>
                                            {/if}
                                          </span>
                                          <code class="ai-cmd-text"><span class="ai-cmd-kind ai-cmd-kind-tool">Tool</span>{tc.serverName} -> {tc.toolName}</code>
                                        </div>
                                        {#if tc.output}
                                          <pre class="ai-cmd-output">{tc.output.length > 500 ? tc.output.substring(0, 500) + '...' : tc.output}</pre>
                                        {/if}
                                      </div>
                                    {/each}
                                  </div>
                                {/if}
                                {#if block.commands.length > 0}
                                  {#if block.toolCalls.length > 0}
                                    <div class="ai-thread-section-heading">Shell commands</div>
                                  {/if}
                                  <div class="ai-block-cmds">
                                    {#each block.commands as cmd, ci}
                                      <div class="ai-cmd-entry">
                                        <div class="ai-cmd-row" class:ai-cmd-running={cmd.status === 'running'} class:ai-cmd-done={cmd.status === 'done'} class:ai-cmd-error={cmd.status === 'error'} class:ai-cmd-input={cmd.status === 'input'}>
                                          <span class="ai-cmd-indicator">
                                            {#if cmd.status === 'running'}
                                              <span class="ai-spinner-sm"></span>
                                            {:else if cmd.status === 'done'}
                                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="var(--term-green)" stroke-width="2.5" fill="none"><polyline points="20 6 9 17 4 12"></polyline></svg>
                                            {:else if cmd.status === 'error'}
                                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="var(--term-red)" stroke-width="2.5" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                                            {:else if cmd.status === 'input'}
                                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="var(--term-yellow)" stroke-width="2.5" fill="none"><circle cx="12" cy="12" r="9"></circle><path d="M12 7v5"></path><circle cx="12" cy="16.5" r="0.8" fill="var(--term-yellow)" stroke="none"></circle></svg>
                                            {:else}
                                              <span class="ai-cmd-num">{ci + 1}</span>
                                            {/if}
                                          </span>
                                          <code class="ai-cmd-text">
                                            {#if cmd.text.startsWith('[AI] ')}
                                              <span class="ai-cmd-kind ai-cmd-kind-note">Note</span>{cmd.text.substring(5)}
                                            {:else if block.toolCalls.length > 0}
                                              <span class="ai-cmd-kind ai-cmd-kind-shell">Shell</span>{cmd.text}
                                            {:else}
                                              {cmd.text}
                                            {/if}
                                          </code>
                                        </div>
                                        {#if cmd.output}
                                          <pre class="ai-cmd-output">{cmd.output}</pre>
                                        {/if}
                                      </div>
                                    {/each}
                                  </div>
                                {/if}
                              </div>

                              {#if block.phase === 'commands'}
                                <div class="ai-block-actions ai-block-actions-end" style="padding: 8px 12px;">
                                  <button class="ai-action-btn ai-action-btn-muted" onclick={cancelPendingCommand}>Dismiss</button>
                                  <button class="ai-action-btn" onclick={copyPendingToTerminal}>
                                    <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
                                    Edit
                                  </button>
                                  <button class="ai-action-btn ai-action-btn-primary" onclick={executePendingCommand}>
                                    <svg viewBox="0 0 24 24" width="13" height="13" stroke="currentColor" stroke-width="2" fill="none"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                                    Run
                                  </button>
                                </div>
                              {/if}
                            </div>
                          {/if}

                          <!-- Answer section -->
                          {#if block.phase === 'needs_input'}
                            <div class="ai-thread-answer ai-thread-answer-input">
                              <div class="ai-thread-label ai-thread-label-ai">
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path></svg>
                                Drover
                              </div>
                              <div class="ai-thread-answer-head">
                                <div class="ai-thread-answer-title">Input required</div>
                                <div class="ai-thread-answer-meta">Shell waiting</div>
                              </div>
                              <div class="ai-thread-answer-text markdown-body">{@html renderAiAnswer(block.response)}</div>
                            </div>
                          {:else if block.phase === 'answered'}
                            <div class="ai-thread-answer">
                              <div class="ai-thread-label ai-thread-label-ai">
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path></svg>
                                Drover
                              </div>
                              <div class="ai-thread-answer-head">
                                <div class="ai-thread-answer-title">Answer</div>
                                {#if getAiExecutedCount(block) > 0}
                                  <div class="ai-thread-answer-meta">{getAiExecutedCount(block)} step{getAiExecutedCount(block) > 1 ? 's' : ''}</div>
                                {/if}
                              </div>
                              <div class="ai-thread-answer-text markdown-body">{@html renderAiAnswer(block.response)}</div>
                            </div>
                          {:else if block.phase === 'executing' || block.phase === 'retrying'}
                            <!-- handled by status above or commands section -->
                          {:else if block.phase === 'done'}
                            <div class="ai-thread-answer ai-thread-answer-done">
                              <div class="ai-thread-label ai-thread-label-ai">
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path></svg>
                                Drover
                              </div>
                              <div class="ai-thread-answer-head">
                                <div class="ai-thread-answer-title">Execution complete</div>
                                {#if getAiExecutedCount(block) > 0}
                                  <div class="ai-thread-answer-meta">{getAiExecutedCount(block)} step{getAiExecutedCount(block) > 1 ? 's' : ''}</div>
                                {/if}
                              </div>
                              <div class="ai-thread-answer-text ai-thread-answer-text-muted">Commands executed successfully.</div>
                            </div>
                          {/if}
                        {/if}
                      </div>
                      {/if}
                    {/each}
                  </div>

                  <!-- Input Area -->
                  <div class="input-area">
                    <div class="input-path-header">
                      <button class="input-path-icon-btn" class:active={fileExplorerOpen} onclick={toggleFileExplorer} title="File Explorer" aria-label="File Explorer">
                        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
                      </button>
                      <span class="input-path-text" title={tab.cwd}>{tab.cwd}</span>
                      {#if tab.commandRunning}
                        <span class="input-runtime-state" class:awaiting={tab.awaitingInput}>
                          <span class="input-runtime-dot"></span>
                          {tab.awaitingInput ? 'Awaiting input' : 'Running'}
                        </span>
                      {/if}
                    </div>
                    <div class="input-field-wrapper">
                      {#if agenticMode && aiAttachments.length > 0}
                        <div class="ai-attachments-bar">
                          {#each aiAttachments as attachment, i}
                            <div class="ai-attachment-chip">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path><polyline points="13 2 13 9 20 9"></polyline></svg>
                              <span class="ai-attachment-name" title={attachment.path}>{attachment.path ? relativeFilePath(attachment.path) : attachment.name}</span>
                              <button class="ai-attachment-remove" onclick={() => removeAttachment(i)} title="Remove" aria-label="Remove attachment">
                                <svg viewBox="0 0 24 24" width="10" height="10" stroke="currentColor" stroke-width="2" fill="none"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                              </button>
                            </div>
                          {/each}
                          <button class="ai-attachment-clear" onclick={clearAllAttachments} title="Clear all">
                            Clear all
                          </button>
                        </div>
                      {/if}
                      {#if slashMenuOpen && savedCommands.length > 0}
                        {@const filtered = getFilteredSlashCommands()}
                        {#if filtered.length > 0}
                          <div class="slash-menu">
                            {#each filtered as cmd, i}
                              <button
                                class="slash-menu-item"
                                class:active={i === slashMenuIndex}
                                onmouseenter={() => { slashMenuIndex = i; }}
                                onclick={() => selectSlashCommand(cmd)}
                              >
                                <span class="slash-menu-name">/{cmd.name}</span>
                                <span class="slash-menu-cmd">{cmd.command}</span>
                                <span class="slash-menu-badge" class:ssh={cmd.category === 'ssh'}>{cmd.category}</span>
                              </button>
                            {/each}
                          </div>
                        {/if}
                      {/if}
                      {#if atMenuOpen && agenticMode}
                        <div class="at-menu">
                          {#if atMenuLoading}
                            <div class="at-menu-loading">Searching...</div>
                          {:else if atFileSuggestions.length > 0}
                            {#each atFileSuggestions as file, i}
                              <button
                                class="at-menu-item"
                                class:active={i === atMenuIndex}
                                onmouseenter={() => { atMenuIndex = i; }}
                                onclick={() => selectAtFile(file)}
                              >
                                <span class="at-menu-icon">
                                  {#if file.isDir}
                                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
                                  {:else}
                                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path><polyline points="13 2 13 9 20 9"></polyline></svg>
                                  {/if}
                                </span>
                                <span class="at-menu-name">{file.name}</span>
                                {#if file.path}
                                  <span class="at-menu-path">{relativeFilePath(file.path)}</span>
                                {/if}
                              </button>
                            {/each}
                          {:else if atFilter.length > 0}
                            <div class="at-menu-empty">No files found matching "{atFilter}"</div>
                          {:else}
                            <div class="at-menu-hint">Type to search files...</div>
                          {/if}
                        </div>
                      {/if}
                      {#if autocompleteSuggestion && tab.inputValue && !agenticMode}
                        <div class="autocomplete-ghost">{autocompleteSuggestion}</div>
                      {/if}
                      <textarea
                        class="input-field"
                        bind:this={paneInputEls[paneLeaf.tabId]}
                        bind:value={tab.inputValue}
                        onfocus={() => {
                          if (paneLayout) {
                            const pane = getAllLeaves(paneLayout.root).find(leaf => leaf.tabId === paneLeaf.tabId);
                            if (pane) handleFocusPane(pane.id);
                          }
                        }}
                        onkeydown={handleKeydown}
                        onpaste={(e) => handleInputPaste(e, tab)}
                        oninput={(e) => {
                          const val = tab.inputValue;
                          if (!agenticMode && val.startsWith('/') && savedCommands.length > 0 && !tab.commandRunning) {
                            slashFilter = val.slice(1);
                            slashMenuOpen = true;
                            slashMenuIndex = 0;
                            autocompleteSuggestion = '';
                          } else {
                            if (slashMenuOpen) closeSlashMenu();
                            debouncedSmartComplete(val);
                          }
                          // @ mention file picker (only in agent mode)
                          if (agenticMode) {
                            const atMatch = val.match(/@([\w./-]*)$/);
                            if (atMatch) {
                              atFilter = atMatch[1];
                              atMenuOpen = true;
                              atMenuIndex = 0;
                              if (atFilter.length > 0) {
                                searchFilesForAtMenu(atFilter);
                              } else {
                                atFileSuggestions = [];
                              }
                            } else {
                              if (atMenuOpen) closeAtMenu();
                            }
                          }
                          autoResizeInput(e.currentTarget as HTMLTextAreaElement);
                        }}
                        placeholder={agenticMode ? 'Describe what you want to do...' : tab.awaitingInput ? 'Interactive prompt active — type response and press Enter' : tab.commandRunning ? 'Command running — input is sent directly to process' : 'Type a command...'}
                        rows="1"
                        spellcheck="false"
                        autocomplete="off"
                        data-autocorrect="off"
                        data-autocapitalize="off"
                        disabled={aiLoading}
                      ></textarea>
                    </div>
                    <div class="input-toolbar">
                      <div class="input-toolbar-left">
                        <div class="mode-switch">
                          <button class="mode-switch-btn" class:active={!agenticMode} onclick={() => { agenticMode = false; pendingCommand = ''; }} title="Terminal mode" aria-label="Terminal mode">
                            Terminal
                          </button>
                          <button class="mode-switch-btn" class:active={agenticMode} onclick={async () => { if (!aiAccountId || !aiApiToken) { openSettings(); return; } agenticMode = true; pendingCommand = ''; await refreshAiSessions(); }} title="AI mode" aria-label="AI mode">
                            Agent
                          </button>
                        </div>
                        {#if agenticMode}
                          <!-- AI Attachments -->
                          <button class="toolbar-icon-btn" onclick={handleAttachFile} title="Attach files" aria-label="Attach files">
                            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"></path></svg>
                            {#if aiAttachments.length > 0}
                              <span class="toolbar-badge">{aiAttachments.length}</span>
                            {/if}
                          </button>
                          <!-- AI Model Selector -->
                          <div class="tools-dropdown-wrapper">
                            <button class="ai-model-selector-btn" class:active={aiModelDropdownOpen} onclick={(e) => { e.stopPropagation(); aiModelDropdownOpen = !aiModelDropdownOpen; toolsDropdownOpen = false; sshDropdownOpen = false; }} title="Select AI model" aria-label="Select AI model">
                              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 1v6m0 6v6M5.64 5.64l4.24 4.24m4.24 4.24l4.24 4.24M1 12h6m6 0h6M5.64 18.36l4.24-4.24m4.24-4.24l4.24-4.24"></path></svg>
                              <span class="ai-model-selector-text">{getSelectedModelName()}</span>
                              <svg viewBox="0 0 24 24" width="10" height="10" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                            </button>
                            {#if aiModelDropdownOpen}
                              <div class="ai-model-dropdown">
                                <div class="ai-model-dropdown-header">Cloudflare</div>
                                {#each AI_MODELS.filter(m => m.provider === 'cloudflare') as model}
                                  <button
                                    class="ai-model-item"
                                    class:active={selectedAiModel === model.id}
                                    class:disabled={!aiApiToken}
                                    onclick={() => { if (aiApiToken) selectAiModel(model.id); }}
                                    title={!aiApiToken ? 'Configure Kuratchi API in Settings' : ''}
                                  >
                                    <div class="ai-model-name">{model.name}</div>
                                    <div class="ai-model-desc">{model.description}</div>
                                    {#if selectedAiModel === model.id}
                                      <svg class="ai-model-check" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="20 6 9 17 4 12"></polyline></svg>
                                    {/if}
                                  </button>
                                {/each}
                                <div class="ai-model-dropdown-header" style="margin-top: 4px;">Google Gemini</div>
                                {#each AI_MODELS.filter(m => m.provider === 'gemini') as model}
                                  <button
                                    class="ai-model-item"
                                    class:active={selectedAiModel === model.id}
                                    class:disabled={!geminiApiKey}
                                    onclick={() => { if (geminiApiKey) selectAiModel(model.id); }}
                                    title={!geminiApiKey ? 'Configure Gemini API key in Settings' : ''}
                                  >
                                    <div class="ai-model-name">{model.name}</div>
                                    <div class="ai-model-desc">{model.description}</div>
                                    {#if selectedAiModel === model.id}
                                      <svg class="ai-model-check" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><polyline points="20 6 9 17 4 12"></polyline></svg>
                                    {/if}
                                  </button>
                                {/each}
                              </div>
                            {/if}
                          </div>
                          <button class="toolbar-text-btn" class:active={aiSessionsSidebarOpen} onclick={async () => { aiSessionsSidebarOpen = !aiSessionsSidebarOpen; aiModelDropdownOpen = false; toolsDropdownOpen = false; sshDropdownOpen = false; if (aiSessionsSidebarOpen) await refreshAiSessions(); }} title="Recent AI sessions" aria-label="Recent AI sessions">
                            <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
                            <span>Sessions</span>
                          </button>
                        {/if}
                        {#if !agenticMode}
                          <div class="tools-dropdown-wrapper">
                            <button class="toolbar-text-btn" class:active={activeTool !== null} onclick={(e) => { e.stopPropagation(); toolsDropdownOpen = !toolsDropdownOpen; sshDropdownOpen = false; aiModelDropdownOpen = false; }} title="Tools" aria-label="Tools">
                              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path></svg>
                              <span>Tools</span>
                            </button>
                            {#if toolsDropdownOpen}
                            <div class="tools-dropdown-menu">
                              <button class="tools-dropdown-item" class:active={activeTool === 'dig'} onclick={() => toggleTool('dig')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
                                Dig
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'curl'} onclick={() => toggleTool('curl')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path><polyline points="15 3 21 3 21 9"></polyline><line x1="10" y1="14" x2="21" y2="3"></line></svg>
                                Curl
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'openssl'} onclick={() => toggleTool('openssl')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect><path d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg>
                                OpenSSL
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'whois'} onclick={() => toggleTool('whois')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"></path><circle cx="12" cy="10" r="3"></circle></svg>
                                Whois
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'ping'} onclick={() => toggleTool('ping')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline></svg>
                                Ping
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'traceroute'} onclick={() => toggleTool('traceroute')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path></svg>
                                Traceroute
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'portscan'} onclick={() => toggleTool('portscan')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
                                Port Scanner
                              </button>
                              <button class="tools-dropdown-item" class:active={activeTool === 'mcp'} onclick={() => toggleTool('mcp')}>
                                <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a4 4 0 0 1 4 4v2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2h-2v2a4 4 0 0 1-8 0v-2H6a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2h2V6a4 4 0 0 1 4-4z"></path></svg>
                                MCP Explorer
                              </button>
                            </div>
                          {/if}
                          </div>
                        {/if}
                        {#if sshSessions.length > 0}
                          <div class="tools-dropdown-wrapper">
                            <button class="toolbar-icon-btn" class:active={sshDropdownOpen} onclick={(e) => { e.stopPropagation(); sshDropdownOpen = !sshDropdownOpen; toolsDropdownOpen = false; }} title="SSH Sessions" aria-label="SSH Sessions">
                              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="2"></rect><path d="M7 8l4 4-4 4"></path><line x1="13" y1="16" x2="17" y2="16"></line></svg>
                            </button>
                            {#if sshDropdownOpen}
                              <div class="tools-dropdown-menu ssh-dropdown-menu">
                                {#each sshSessions as session}
                                  <button class="tools-dropdown-item" onclick={() => connectSshSession(session)}>
                                    <span class="ssh-session-nick">{session.nickname}</span>
                                    <span class="ssh-session-cmd">{session.command}</span>
                                  </button>
                                {/each}
                              </div>
                            {/if}
                          </div>
                        {/if}
                      </div>
                      <button class="toolbar-text-btn" onclick={openSettings} title="Settings" aria-label="Settings">
                        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                        <span>Settings</span>
                      </button>
                      {#if tab.commandRunning}
                        <div class="input-runtime-hint">
                          {tab.awaitingInput ? 'Interactive prompt: Enter submits · Ctrl+C cancels' : 'Command running · Ctrl+C to interrupt'}
                        </div>
                      {/if}
                    </div>
                  </div>
                </div>
              {/if}
              </div>
              {/each}
            {:else}
              {@const container = node as PaneContainer}
              <div class="pane-container"
                class:horizontal={container.direction === 'horizontal'}
                class:vertical={container.direction === 'vertical'}
                data-container-id={container.id}
              >
                {#each container.children as child, i (child.id)}
                  {#if i > 0}
                    <div class="split-divider"
                      onmousedown={(e) => handleResizeStart(e, i - 1, container)}
                      role="separator"
                      aria-orientation={container.direction === 'horizontal' ? 'horizontal' : 'vertical'}
                    ></div>
                  {/if}
                  <div class="pane-wrapper" style="flex: {container.sizes[i] || 50}">
                    {@render renderPane(child)}
                  </div>
                {/each}
              </div>
            {/if}
          {/snippet}
          {@render renderPane(paneLayout.root)}
        {/if}

        <!-- Tools pane (right side) -->
        {#if activeTool}
          <div class="split-divider" onmousedown={handleRightPaneResizeStart} role="separator" aria-orientation="vertical"></div>
          <aside class="tools-pane" style="width: {rightPaneWidthPct}%; flex: none;" role="none" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
            <div class="tools-pane-header">
              <span class="tools-pane-title">{({'dig':'Dig','curl':'Curl','openssl':'OpenSSL','whois':'Whois','ping':'Ping','traceroute':'Traceroute','portscan':'Port Scanner','mcp':'MCP Explorer'})[activeTool ?? ''] ?? 'Tool'}</span>
              <button class="tools-pane-close" onclick={closeTool} title="Close tool" aria-label="Close tool">&times;</button>
            </div>
            <div class="tools-pane-content">
              {#if activeTool === 'dig'}
                <DigApp />
              {:else if activeTool === 'curl'}
                <CurlApp />
              {:else if activeTool === 'openssl'}
                <OpensslApp />
              {:else if activeTool === 'whois'}
                <WhoisApp />
              {:else if activeTool === 'ping'}
                <PingApp />
              {:else if activeTool === 'traceroute'}
                <TracerouteApp />
              {:else if activeTool === 'portscan'}
                <PortScanApp />
              {:else if activeTool === 'mcp'}
                <McpExplorer />
              {/if}
            </div>
          </aside>
        {/if}

        <!-- Right pane: Editor or Preview -->
        {#if rightPane === 'editor'}
          <div class="split-divider" onmousedown={handleRightPaneResizeStart} role="separator" aria-orientation="vertical"></div>
          <div class="right-pane" style="width: {rightPaneWidthPct}%; flex: none;" onclick={(e) => e.stopPropagation()}>
            <div class="right-pane-header">
              <span class="right-pane-title">{editorFileName}{editorDirty ? ' •' : ''}</span>
              <div class="right-pane-actions">
                {#if editorSaveStatus === 'saving'}
                  <span class="right-pane-badge">Saving...</span>
                {:else if editorSaveStatus === 'saved'}
                  <span class="right-pane-badge right-pane-badge-success">Saved</span>
                {:else if editorSaveStatus === 'error'}
                  <span class="right-pane-badge right-pane-badge-error">Failed</span>
                {/if}
                <button class="right-pane-btn" onclick={() => editorViewRef?.saveFile()} title={`Save (${shortcutLabel('S')})`} aria-label="Save" disabled={!editorDirty}>
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline points="7 3 7 8 15 8"></polyline></svg>
                </button>
                {#if isMarkdownFile(editorFileName)}
                  <button class="right-pane-btn" onclick={switchToMarkdownPreview} title="Preview" aria-label="Preview Markdown">
                    <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                  </button>
                {/if}
                <button class="right-pane-btn" onclick={closeEditor} title="Close" aria-label="Close">&times;</button>
              </div>
            </div>
            <EditorView
              bind:this={editorViewRef}
              bind:filePath={editorFilePath}
              bind:fileName={editorFileName}
              bind:content={editorContent}
              bind:dirty={editorDirty}
              bind:saveStatus={editorSaveStatus}
              bind:lang={editorLang}
              bind:loading={editorLoading}
              bind:error={editorError}
              sshTarget={editorSshTarget}
              onClose={closeEditor}
            />
          </div>
        {:else if rightPane === 'preview'}
          <div class="split-divider" onmousedown={handleRightPaneResizeStart} role="separator" aria-orientation="vertical"></div>
          <div class="right-pane" style="width: {rightPaneWidthPct}%; flex: none;" onclick={(e) => e.stopPropagation()}>
            <div class="right-pane-header">
              <span class="right-pane-title">{previewFileName}</span>
              <div class="right-pane-actions">
                <button class="right-pane-btn" onclick={closePreview} title="Close" aria-label="Close">&times;</button>
              </div>
            </div>
            <div class="preview-pane">
              {#if previewLoading}
                <div class="preview-loading">Loading preview...</div>
              {:else if previewUrl}
                <div class="preview-container">
                  <img src={previewUrl} alt={previewFileName} class="preview-image" />
                </div>
              {:else}
                <div class="preview-error">Failed to load preview</div>
              {/if}
            </div>
          </div>
        {:else if rightPane === 'markdown'}
          <div class="split-divider" onmousedown={handleRightPaneResizeStart} role="separator" aria-orientation="vertical"></div>
          <div class="right-pane" style="width: {rightPaneWidthPct}%; flex: none;" onclick={(e) => e.stopPropagation()}>
            <div class="right-pane-header">
              <span class="right-pane-title">{mdPreviewFileName}</span>
              <div class="right-pane-actions">
                <button class="right-pane-btn" onclick={openMarkdownInEditor} title="Edit" aria-label="Edit">
                  <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"></path></svg>
                </button>
                <button class="right-pane-btn" onclick={closeMarkdownPreview} title="Close" aria-label="Close">&times;</button>
              </div>
            </div>
            <MarkdownPreview
              bind:this={mdPreviewRef}
              bind:filePath={mdPreviewFilePath}
              bind:fileName={mdPreviewFileName}
              bind:loading={mdPreviewLoading}
              bind:error={mdPreviewError}
              sshTarget={mdPreviewSshTarget}
              onClose={closeMarkdownPreview}
              onEdit={openMarkdownInEditor}
            />
          </div>
        {:else if rightPane === 'diff' && pendingCodeEdit}
          <div class="split-divider" onmousedown={handleRightPaneResizeStart} role="separator" aria-orientation="vertical"></div>
          <div class="right-pane" style="width: {rightPaneWidthPct}%; flex: none;" onclick={(e) => e.stopPropagation()}>
            <DiffViewer
              fileName={pendingCodeEdit.file_name}
              diffLines={pendingCodeEdit.diff_lines}
              summary={pendingCodeEdit.summary}
              onApply={() => applyCodeEdit()}
              onReject={() => rejectCodeEdit()}
              applying={applyingCodeEdit}
            />
          </div>
        {/if}
      </div>
    {/if}

    <!-- Pane Context Menu -->
    {#if showContextMenu}
      <div class="context-menu-backdrop" onclick={dismissContextMenu} role="presentation"></div>
      <div class="pane-context-menu" style="left: {contextMenuX}px; top: {contextMenuY}px" role="menu">
        {#if contextMenuHasSelection}
          <button class="pane-context-item" onclick={contextCopy} role="menuitem">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="5" y="5" width="9" height="9" rx="1.5"/><path d="M3 11V3a1.5 1.5 0 0 1 1.5-1.5H11"/></svg>
            Copy
          </button>
          <button class="pane-context-item" onclick={contextPaste} role="menuitem">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="1" width="10" height="14" rx="1.5"/><line x1="6" y1="5" x2="10" y2="5"/><line x1="6" y1="8" x2="10" y2="8"/></svg>
            Paste
          </button>
          <div class="pane-context-separator"></div>
        {:else}
          <button class="pane-context-item" onclick={contextPaste} role="menuitem">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="1" width="10" height="14" rx="1.5"/><line x1="6" y1="5" x2="10" y2="5"/><line x1="6" y1="8" x2="10" y2="8"/></svg>
            Paste
          </button>
          <div class="pane-context-separator"></div>
        {/if}
        <button class="pane-context-item" onclick={() => contextSplit('right')} role="menuitem">
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="14" height="14" rx="2"/><line x1="8" y1="1" x2="8" y2="15"/></svg>
          Split Right
            <span class="pane-context-shortcut">{splitShortcutLabel('right')}</span>
        </button>
        <button class="pane-context-item" onclick={() => contextSplit('bottom')} role="menuitem">
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="14" height="14" rx="2"/><line x1="1" y1="8" x2="15" y2="8"/></svg>
          Split Down
            <span class="pane-context-shortcut">{splitShortcutLabel('bottom')}</span>
        </button>
        <button class="pane-context-item" onclick={() => contextSplit('left')} role="menuitem">
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="14" height="14" rx="2"/><line x1="8" y1="1" x2="8" y2="15"/></svg>
          Split Left
            <span class="pane-context-shortcut">{splitShortcutLabel('left')}</span>
        </button>
        <button class="pane-context-item" onclick={() => contextSplit('top')} role="menuitem">
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="14" height="14" rx="2"/><line x1="1" y1="8" x2="15" y2="8"/></svg>
          Split Up
            <span class="pane-context-shortcut">{splitShortcutLabel('top')}</span>
        </button>
        {#if paneLayout && getAllLeaves(paneLayout.root).length > 1}
          <div class="pane-context-separator"></div>
          <button class="pane-context-item danger" onclick={contextClose} role="menuitem">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5"><line x1="4" y1="4" x2="12" y2="12"/><line x1="12" y1="4" x2="4" y2="12"/></svg>
            Close Pane
            <span class="pane-context-shortcut">{shortcutLabel('W')}</span>
          </button>
        {/if}
      </div>
    {/if}

    <!-- Terminal File Context Menu -->
    {#if terminalFileCtxMenu}
      <div 
        class="terminal-file-ctx-menu" 
        style="left: {terminalFileCtxMenu.x}px; top: {terminalFileCtxMenu.y}px;"
        onclick={(e) => e.stopPropagation()}
      >
        <button class="ctx-menu-item" onclick={addTerminalFileToAgent}>
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none">
            <path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path>
          </svg>
          <span>Add to Agent</span>
        </button>
        <button class="ctx-menu-item" onclick={() => { 
          if (terminalFileCtxMenu) {
            const { filePath, fileName, tab } = terminalFileCtxMenu;
            closeTerminalFileCtxMenu();
            editorSshTarget = tab.sshTarget;
            rightPane = 'editor';
            activeTool = null;
            tick().then(() => editorViewRef?.openFile(filePath, fileName));
          }
        }}>
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none">
            <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
            <polyline points="13 2 13 9 20 9"></polyline>
          </svg>
          <span>Open in Editor</span>
        </button>
      </div>
    {/if}

    <!-- Status Bar -->
    <footer class="status-bar">
      <div class="status-item">
        <span class="status-dot" class:disconnected={!activeTab()?.connected}></span>
        <span>{activeTab()?.connected ? 'Connected' : 'Disconnected'}</span>
      </div>
      <div class="status-item">
        <span>{rightPane === 'editor' ? 'Editor' : rightPane === 'preview' ? 'Preview' : rightPane === 'markdown' ? 'Markdown Preview' : agenticMode ? 'AI Mode' : 'Terminal'}</span>
      </div>
      <div class="status-item">
        <span>{rightPane === 'editor' ? editorLang : getLocalShellLabel()}</span>
      </div>
      <div class="status-spacer"></div>
      <div class="status-item">
        <span>Drover v0.1.0</span>
      </div>
    </footer>
  </div>
</div>

