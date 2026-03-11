<script lang="ts">
  import { THEMES, applyThemeCssVars, type TerminalTheme } from '$lib/theme';

  interface SshSession { id: string; nickname: string; command: string; }
  interface SavedCommand { id: string; name: string; command: string; category: 'ssh' | 'quick'; }
  interface McpServerEntry { name: string; server_url: string; auth_token?: string; auth_command?: string; }

  let {
    aiAccountId = $bindable(''),
    aiApiToken = $bindable(''),
    activeThemeId = $bindable('dracula'),
    customTheme = $bindable<TerminalTheme>({ ...THEMES.custom }),
    sshSessions = [],
    savedCommands = [],
    mcpServers = [],
    onSaveSettings,
    onRevokeCredentials,
    onSaveTheme,
    onAddSshSession,
    onRemoveSshSession,
    onAddSavedCommand,
    onRemoveSavedCommand,
    onAddMcpServer,
    onRemoveMcpServer,
  }: {
    aiAccountId: string;
    aiApiToken: string;
    activeThemeId: string;
    customTheme: TerminalTheme;
    sshSessions: SshSession[];
    savedCommands: SavedCommand[];
    mcpServers: McpServerEntry[];
    onSaveSettings: (accountId: string, apiToken: string) => Promise<void>;
    onRevokeCredentials: () => Promise<void>;
    onSaveTheme: () => Promise<void>;
    onAddSshSession: (nickname: string, command: string) => void;
    onRemoveSshSession: (id: string) => void;
    onAddSavedCommand: (name: string, command: string, category: 'ssh' | 'quick') => void;
    onRemoveSavedCommand: (id: string) => void;
    onAddMcpServer: (name: string, serverUrl: string, authToken: string, authCommand: string) => void;
    onRemoveMcpServer: (name: string) => void;
  } = $props();

  let settingsTab = $state<'ai' | 'theme' | 'ssh' | 'commands' | 'mcp' | 'about'>('ai');
  let newCmdName = $state('');
  let newCmdCommand = $state('');
  let newCmdCategory = $state<'quick' | 'ssh'>('quick');
  let newSshNickname = $state('');
  let newSshCommand = $state('');
  let newMcpName = $state('');
  let newMcpUrl = $state('');
  let newMcpAuthToken = $state('');
  let newMcpAuthCommand = $state('');
  let showMcpAuthToken = $state(false);
  let settingsAccountId = $state(aiAccountId);
  let settingsApiToken = $state(aiApiToken);
  let showToken = $state(false);
  let settingsSaved = $state(false);

  // Sync when props change
  $effect(() => {
    settingsAccountId = aiAccountId;
    settingsApiToken = aiApiToken;
  });

  async function handleSave() {
    await onSaveSettings(settingsAccountId.trim(), settingsApiToken.trim());
    settingsSaved = true;
    setTimeout(() => { settingsSaved = false; }, 2000);
  }

  async function handleRevoke() {
    await onRevokeCredentials();
    settingsAccountId = '';
    settingsApiToken = '';
    showToken = false;
  }

  function selectTheme(id: string) {
    activeThemeId = id;
    applyThemeCssVars(activeThemeId === 'custom' ? customTheme : THEMES[activeThemeId] || THEMES.dracula);
    onSaveTheme();
  }

  function updateCustomColor() {
    applyThemeCssVars(customTheme);
    onSaveTheme();
  }
</script>

<div class="settings-layout">
  <nav class="settings-sidebar">
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'ai'}
      onclick={() => { settingsTab = 'ai'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a2 2 0 0 1 2 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 0 1 7 7h1a1 1 0 0 1 1 1v3a1 1 0 0 1-1 1h-1.27a7 7 0 0 1-12.46 0H6a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1h1a7 7 0 0 1 7-7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 0 1 2-2z"></path><circle cx="9.5" cy="15.5" r="1"></circle><circle cx="14.5" cy="15.5" r="1"></circle></svg>
      <span>AI</span>
    </button>
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'theme'}
      onclick={() => { settingsTab = 'theme'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line></svg>
      <span>Theme</span>
    </button>
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'ssh'}
      onclick={() => { settingsTab = 'ssh'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="2"></rect><path d="M7 8l4 4-4 4"></path><line x1="13" y1="16" x2="17" y2="16"></line></svg>
      <span>SSH</span>
    </button>
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'commands'}
      onclick={() => { settingsTab = 'commands'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
      <span>Commands</span>
    </button>
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'mcp'}
      onclick={() => { settingsTab = 'mcp'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a4 4 0 0 1 4 4v2h2a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2h-2v2a4 4 0 0 1-8 0v-2H6a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2h2V6a4 4 0 0 1 4-4z"></path></svg>
      <span>MCP</span>
    </button>
    <button
      class="settings-nav-item"
      class:active={settingsTab === 'about'}
      onclick={() => { settingsTab = 'about'; }}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="16" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>
      <span>About</span>
    </button>
  </nav>

  <div class="settings-panel">
    {#if settingsTab === 'ai'}
      <section class="settings-section">
        <h2 class="settings-section-title">Cloudflare Workers AI</h2>
        <p class="settings-section-desc">Configure your Cloudflare credentials to enable the AI-powered agentic terminal mode. Your credentials are stored locally on this device.</p>

        <div class="settings-field">
          <label class="settings-label" for="ai-account-id">Account ID</label>
          <input
            id="ai-account-id"
            class="settings-input"
            type="text"
            bind:value={settingsAccountId}
            placeholder="Enter your Cloudflare Account ID"
            spellcheck="false"
          />
          <span class="settings-hint">Found in your Cloudflare Dashboard under Workers AI</span>
        </div>

        <div class="settings-field">
          <label class="settings-label" for="ai-api-token">API Token</label>
          <div class="settings-input-group">
            <input
              id="ai-api-token"
              class="settings-input"
              type={showToken ? 'text' : 'password'}
              bind:value={settingsApiToken}
              placeholder="Enter your Workers AI API token"
              spellcheck="false"
            />
            <button class="settings-reveal-btn" onclick={() => { showToken = !showToken; }} title={showToken ? 'Hide token' : 'Show token'}>
              {showToken ? 'Hide' : 'Show'}
            </button>
          </div>
          <span class="settings-hint">Requires Workers AI permissions</span>
        </div>

        {#if aiAccountId && aiApiToken}
          <div class="settings-status settings-status-active">
            <span class="settings-status-dot active"></span>
            <span>Credentials configured — AI mode available</span>
          </div>
        {:else}
          <div class="settings-status settings-status-inactive">
            <span class="settings-status-dot"></span>
            <span>No credentials — AI mode disabled</span>
          </div>
        {/if}

        <div class="settings-actions">
          <button class="settings-btn-save" onclick={handleSave} disabled={!settingsAccountId.trim() || !settingsApiToken.trim()}>
            {settingsSaved ? '✓ Saved' : 'Save Credentials'}
          </button>
          {#if aiAccountId || aiApiToken}
            <button class="settings-btn-revoke" onclick={handleRevoke}>
              Revoke Credentials
            </button>
          {/if}
        </div>
      </section>

    {:else if settingsTab === 'theme'}
      <section class="settings-section">
        <h2 class="settings-section-title">Terminal Theme</h2>
        <p class="settings-section-desc">Customize prompt and output colors. Changes apply immediately.</p>

        <div class="theme-picker">
          {#each Object.values(THEMES) as theme}
            <button
              class="theme-option"
              class:active={activeThemeId === theme.id}
              onclick={() => selectTheme(theme.id)}
            >
              <div class="theme-preview">
                <span style="color:{theme.promptUser}">user</span><span style="color:{theme.promptHost}">@host</span>
                <span style="color:{theme.promptPath}"> ~/path</span>
                <span style="color:{theme.promptSymbol}"> %</span>
                <span style="color:{theme.promptCommand}"> ls</span>
              </div>
              <span class="theme-name">{theme.name}</span>
            </button>
          {/each}
        </div>

        {#if activeThemeId === 'custom'}
          <div class="theme-custom-editor">
            <div class="theme-color-row">
              <span class="theme-color-label">Username</span>
              <input type="color" bind:value={customTheme.promptUser} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.promptUser}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Hostname</span>
              <input type="color" bind:value={customTheme.promptHost} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.promptHost}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Path</span>
              <input type="color" bind:value={customTheme.promptPath} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.promptPath}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Symbol (%)</span>
              <input type="color" bind:value={customTheme.promptSymbol} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.promptSymbol}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Command</span>
              <input type="color" bind:value={customTheme.promptCommand} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.promptCommand}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Directory</span>
              <input type="color" bind:value={customTheme.directory} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.directory}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Executable</span>
              <input type="color" bind:value={customTheme.executable} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.executable}</span>
            </div>
            <div class="theme-color-row">
              <span class="theme-color-label">Symlink</span>
              <input type="color" bind:value={customTheme.symlink} oninput={updateCustomColor} />
              <span class="theme-color-hex">{customTheme.symlink}</span>
            </div>
          </div>
        {/if}
      </section>

    {:else if settingsTab === 'ssh'}
      <section class="settings-section">
        <h2 class="settings-section-title">SSH Sessions</h2>
        <p class="settings-section-desc">Save your frequently used SSH connections for quick access from the terminal input bar.</p>

        {#if sshSessions.length > 0}
          <div class="ssh-sessions-list">
            {#each sshSessions as session}
              <div class="ssh-session-row">
                <div class="ssh-session-info">
                  <span class="ssh-session-nickname">{session.nickname}</span>
                  <span class="ssh-session-command">{session.command}</span>
                </div>
                <button class="ssh-session-remove" onclick={() => onRemoveSshSession(session.id)} title="Remove" aria-label="Remove session">&times;</button>
              </div>
            {/each}
          </div>
        {:else}
          <div class="settings-status settings-status-inactive">
            <span class="settings-status-dot"></span>
            <span>No saved sessions</span>
          </div>
        {/if}

        <div class="ssh-add-form">
          <h3 class="ssh-add-title">Add Session</h3>
          <div class="settings-field">
            <label class="settings-label" for="ssh-nickname">Nickname</label>
            <input
              id="ssh-nickname"
              class="settings-input"
              type="text"
              bind:value={newSshNickname}
              placeholder="e.g. My DO VM"
              spellcheck="false"
            />
          </div>
          <div class="settings-field">
            <label class="settings-label" for="ssh-command">SSH Command</label>
            <input
              id="ssh-command"
              class="settings-input"
              type="text"
              bind:value={newSshCommand}
              placeholder="e.g. ssh root@192.168.1.100"
              spellcheck="false"
            />
            <span class="settings-hint">The full command that will be run in the terminal</span>
          </div>
          <div class="settings-actions">
            <button
              class="settings-btn-save"
              disabled={!newSshNickname.trim() || !newSshCommand.trim()}
              onclick={() => { onAddSshSession(newSshNickname.trim(), newSshCommand.trim()); newSshNickname = ''; newSshCommand = ''; }}
            >
              Add Session
            </button>
          </div>
        </div>
      </section>

    {:else if settingsTab === 'commands'}
      <section class="settings-section">
        <h2 class="settings-section-title">Saved Commands</h2>
        <p class="settings-section-desc">Save frequently used commands for quick access. Type <code>/name</code> in the terminal input to search and run them instantly.</p>

        {#if savedCommands.length > 0}
          <div class="ssh-sessions-list">
            {#each savedCommands as cmd}
              <div class="ssh-session-row">
                <div class="ssh-session-info">
                  <span class="ssh-session-nickname">/{cmd.name} <span class="slash-menu-badge" class:ssh={cmd.category === 'ssh'}>{cmd.category}</span></span>
                  <span class="ssh-session-command">{cmd.command}</span>
                </div>
                <button class="ssh-session-remove" onclick={() => onRemoveSavedCommand(cmd.id)} title="Remove" aria-label="Remove command">&times;</button>
              </div>
            {/each}
          </div>
        {:else}
          <div class="settings-status settings-status-inactive">
            <span class="settings-status-dot"></span>
            <span>No saved commands</span>
          </div>
        {/if}

        <div class="ssh-add-form">
          <h3 class="ssh-add-title">Add Command</h3>
          <div class="settings-field">
            <label class="settings-label" for="cmd-name">Name</label>
            <input
              id="cmd-name"
              class="settings-input"
              type="text"
              bind:value={newCmdName}
              placeholder="e.g. deploy-prod"
              spellcheck="false"
            />
            <span class="settings-hint">Used as the slash command trigger (e.g. /deploy-prod)</span>
          </div>
          <div class="settings-field">
            <label class="settings-label" for="cmd-command">Command</label>
            <input
              id="cmd-command"
              class="settings-input"
              type="text"
              bind:value={newCmdCommand}
              placeholder="e.g. ssh root@192.168.1.1 or kubectl get pods -n production"
              spellcheck="false"
            />
            <span class="settings-hint">The full command that will be populated in the terminal</span>
          </div>
          <div class="settings-field">
            <label class="settings-label" for="cmd-category">Category</label>
            <div class="cmd-category-picker" id="cmd-category">
              <button class="cmd-category-btn" class:active={newCmdCategory === 'quick'} onclick={() => { newCmdCategory = 'quick'; }}>Quick</button>
              <button class="cmd-category-btn" class:active={newCmdCategory === 'ssh'} onclick={() => { newCmdCategory = 'ssh'; }}>SSH</button>
            </div>
          </div>
          <div class="settings-actions">
            <button
              class="settings-btn-save"
              disabled={!newCmdName.trim() || !newCmdCommand.trim()}
              onclick={() => { onAddSavedCommand(newCmdName.trim(), newCmdCommand.trim(), newCmdCategory); newCmdName = ''; newCmdCommand = ''; newCmdCategory = 'quick'; }}
            >
              Add Command
            </button>
          </div>
        </div>
      </section>

    {:else if settingsTab === 'mcp'}
      <section class="settings-section">
        <h2 class="settings-section-title">MCP Servers</h2>
        <p class="settings-section-desc">Connect to Model Context Protocol servers to extend AI capabilities with external tools and data sources.</p>

        {#if mcpServers.length > 0}
          <div class="ssh-sessions-list">
            {#each mcpServers as server}
              <div class="ssh-session-row">
                <div class="ssh-session-info">
                  <span class="ssh-session-nickname">{server.name}</span>
                  <span class="ssh-session-command">{server.server_url}</span>
                  {#if server.auth_token}
                    <span class="mcp-auth-badge">Token</span>
                  {/if}
                  {#if server.auth_command}
                    <span class="mcp-auth-badge">Command</span>
                  {/if}
                </div>
                <button class="ssh-session-remove" onclick={() => onRemoveMcpServer(server.name)} title="Remove" aria-label="Remove server">&times;</button>
              </div>
            {/each}
          </div>
        {:else}
          <div class="settings-status settings-status-inactive">
            <span class="settings-status-dot"></span>
            <span>No MCP servers configured</span>
          </div>
        {/if}

        <div class="ssh-add-form">
          <h3 class="ssh-add-title">Add Server</h3>
          <div class="settings-field">
            <label class="settings-label" for="mcp-name">Name</label>
            <input
              id="mcp-name"
              class="settings-input"
              type="text"
              bind:value={newMcpName}
              placeholder="e.g. my-mcp-server"
              spellcheck="false"
            />
            <span class="settings-hint">A unique identifier for this server</span>
          </div>
          <div class="settings-field">
            <label class="settings-label" for="mcp-url">Server URL</label>
            <input
              id="mcp-url"
              class="settings-input"
              type="text"
              bind:value={newMcpUrl}
              placeholder="e.g. https://my-server.example.com/mcp"
              spellcheck="false"
            />
            <span class="settings-hint">The MCP server's HTTP endpoint</span>
          </div>
          <div class="settings-field">
            <label class="settings-label" for="mcp-auth-token">Auth Token <span class="settings-optional">(optional)</span></label>
            <div class="settings-input-group">
              <input
                id="mcp-auth-token"
                class="settings-input"
                type={showMcpAuthToken ? 'text' : 'password'}
                bind:value={newMcpAuthToken}
                placeholder="Static bearer token"
                spellcheck="false"
              />
              <button class="settings-reveal-btn" onclick={() => { showMcpAuthToken = !showMcpAuthToken; }} title={showMcpAuthToken ? 'Hide' : 'Show'}>
                {showMcpAuthToken ? 'Hide' : 'Show'}
              </button>
            </div>
          </div>
          <div class="settings-field">
            <label class="settings-label" for="mcp-auth-cmd">Auth Command <span class="settings-optional">(optional)</span></label>
            <input
              id="mcp-auth-cmd"
              class="settings-input"
              type="text"
              bind:value={newMcpAuthCommand}
              placeholder="e.g. my-cli auth token"
              spellcheck="false"
            />
            <span class="settings-hint">Shell command that outputs a bearer token to stdout</span>
          </div>
          <div class="settings-actions">
            <button
              class="settings-btn-save"
              disabled={!newMcpName.trim() || !newMcpUrl.trim()}
              onclick={() => { onAddMcpServer(newMcpName.trim(), newMcpUrl.trim(), newMcpAuthToken.trim(), newMcpAuthCommand.trim()); newMcpName = ''; newMcpUrl = ''; newMcpAuthToken = ''; newMcpAuthCommand = ''; }}
            >
              Add Server
            </button>
          </div>
        </div>
      </section>

    {:else if settingsTab === 'about'}
      <section class="settings-section">
        <h2 class="settings-section-title">About</h2>
        <div class="settings-about">
          <div class="settings-about-row">
            <span class="settings-about-label">Version</span>
            <span class="settings-about-value">0.1.0</span>
          </div>
          <div class="settings-about-row">
            <span class="settings-about-label">AI Model</span>
            <span class="settings-about-value">@cf/meta/llama-3.1-8b-instruct</span>
          </div>
          <div class="settings-about-row">
            <span class="settings-about-label">Shell</span>
            <span class="settings-about-value">zsh</span>
          </div>
          <div class="settings-about-row">
            <span class="settings-about-label">Backend</span>
            <span class="settings-about-value">Tauri 2 + Rust</span>
          </div>
          <div class="settings-about-row">
            <span class="settings-about-label">Frontend</span>
            <span class="settings-about-value">SvelteKit + Svelte 5</span>
          </div>
        </div>
      </section>
    {/if}
  </div>
</div>
