<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface McpServer {
    name: string;
    server_url: string;
  }

  interface McpTool {
    name: string;
    description: string | null;
    input_schema: any;
  }

  interface McpContentItem {
    content_type: string;
    text: string | null;
  }

  interface McpToolResult {
    content: McpContentItem[];
    is_error: boolean | null;
  }

  interface McpServerInfo {
    name: string;
    version: string | null;
    protocol_version: string | null;
    capabilities: any;
  }

  let servers = $state<McpServer[]>([]);
  let selectedServer = $state<McpServer | null>(null);
  let serverInfo = $state<McpServerInfo | null>(null);
  let tools = $state<McpTool[]>([]);
  let selectedTool = $state<McpTool | null>(null);
  let toolArgs = $state('{}');
  let toolResult = $state<McpToolResult | null>(null);
  let loading = $state(false);
  let error = $state('');
  let phase = $state<'servers' | 'tools' | 'call'>('servers');

  async function loadServers() {
    error = '';
    try {
      servers = await invoke('mcp_list_servers');
    } catch (e: any) {
      error = e?.toString() ?? 'Failed to load servers';
    }
  }

  async function connectServer(server: McpServer) {
    loading = true;
    error = '';
    selectedServer = server;
    serverInfo = null;
    tools = [];
    selectedTool = null;
    toolResult = null;
    try {
      serverInfo = await invoke('mcp_initialize', {
        serverUrl: server.server_url,
        serverName: server.name,
      });
      const result: McpTool[] = await invoke('mcp_list_tools', {
        serverUrl: server.server_url,
        serverName: server.name,
      });
      tools = result;
      phase = 'tools';
    } catch (e: any) {
      error = e?.toString() ?? 'Failed to connect';
    } finally {
      loading = false;
    }
  }

  function selectTool(tool: McpTool) {
    selectedTool = tool;
    toolResult = null;
    error = '';
    const schema = tool.input_schema;
    if (schema?.properties) {
      const stub: Record<string, any> = {};
      for (const [key, prop] of Object.entries(schema.properties) as [string, any][]) {
        if (prop.type === 'string') stub[key] = '';
        else if (prop.type === 'number') stub[key] = 0;
        else if (prop.type === 'boolean') stub[key] = false;
        else if (prop.type === 'array') stub[key] = [];
        else stub[key] = null;
      }
      toolArgs = JSON.stringify(stub, null, 2);
    } else {
      toolArgs = '{}';
    }
    phase = 'call';
  }

  async function callTool() {
    if (!selectedServer || !selectedTool) return;
    loading = true;
    error = '';
    toolResult = null;
    try {
      const args = JSON.parse(toolArgs);
      toolResult = await invoke('mcp_call_tool', {
        serverUrl: selectedServer.server_url,
        serverName: selectedServer.name,
        toolName: selectedTool.name,
        arguments: args,
      });
    } catch (e: any) {
      error = e?.toString() ?? 'Tool call failed';
    } finally {
      loading = false;
    }
  }

  function goBack() {
    if (phase === 'call') {
      phase = 'tools';
      selectedTool = null;
      toolResult = null;
      error = '';
    } else if (phase === 'tools') {
      phase = 'servers';
      selectedServer = null;
      serverInfo = null;
      tools = [];
      error = '';
    }
  }

  loadServers();
</script>

<div class="mcp-explorer">
  <div class="mcp-breadcrumb">
    {#if phase === 'servers'}
      <span class="mcp-bc-active">Servers</span>
    {:else if phase === 'tools'}
      <button class="mcp-bc-link" onclick={goBack}>Servers</button>
      <span class="mcp-bc-sep">/</span>
      <span class="mcp-bc-active">{selectedServer?.name}</span>
    {:else}
      <button class="mcp-bc-link" onclick={() => { phase = 'servers'; selectedServer = null; }}>Servers</button>
      <span class="mcp-bc-sep">/</span>
      <button class="mcp-bc-link" onclick={goBack}>{selectedServer?.name}</button>
      <span class="mcp-bc-sep">/</span>
      <span class="mcp-bc-active">{selectedTool?.name}</span>
    {/if}
  </div>

  {#if error}
    <div class="mcp-error">{error}</div>
  {/if}

  {#if phase === 'servers'}
    <div class="mcp-section">
      <div class="mcp-section-title">Configured Servers</div>
      {#if servers.length === 0}
        <div class="mcp-empty">
          No MCP servers configured.<br>
          Add servers in <strong>Settings &rarr; MCP</strong>
        </div>
      {:else}
        {#each servers as server}
          <button class="mcp-server-item" onclick={() => connectServer(server)} disabled={loading}>
            <div class="mcp-server-name">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
              {server.name}
            </div>
            <div class="mcp-server-url">{server.server_url}</div>
          </button>
        {/each}
      {/if}
    </div>

  {:else if phase === 'tools'}
    {#if serverInfo}
      <div class="mcp-server-info">
        <span class="mcp-info-badge">Connected</span>
        <span class="mcp-info-detail">{serverInfo.name} {serverInfo.version ?? ''}</span>
        <span class="mcp-info-detail mcp-muted">MCP {serverInfo.protocol_version ?? '?'}</span>
      </div>
    {/if}

    <div class="mcp-section">
      <div class="mcp-section-title">Available Tools ({tools.length})</div>
      {#if loading}
        <div class="mcp-loading">Connecting...</div>
      {:else if tools.length === 0}
        <div class="mcp-empty">No tools available on this server.</div>
      {:else}
        {#each tools as tool}
          <button class="mcp-tool-item" onclick={() => selectTool(tool)}>
            <div class="mcp-tool-name">
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path></svg>
              {tool.name}
            </div>
            {#if tool.description}
              <div class="mcp-tool-desc">{tool.description}</div>
            {/if}
          </button>
        {/each}
      {/if}
    </div>

  {:else if phase === 'call'}
    <div class="mcp-section">
      <div class="mcp-section-title">{selectedTool?.name}</div>
      {#if selectedTool?.description}
        <div class="mcp-tool-full-desc">{selectedTool.description}</div>
      {/if}

      {#if selectedTool?.input_schema?.properties}
        <div class="mcp-schema-info">
          <div class="mcp-schema-title">Parameters</div>
          {#each Object.entries(selectedTool.input_schema.properties) as [key, prop]}
            {@const p = prop as any}
            <div class="mcp-schema-param">
              <code>{key}</code>
              <span class="mcp-schema-type">{p.type ?? 'any'}</span>
              {#if selectedTool.input_schema.required?.includes(key)}
                <span class="mcp-schema-required">required</span>
              {/if}
              {#if p.description}
                <span class="mcp-schema-desc">— {p.description}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      <div class="mcp-call-section">
        <label class="mcp-label">Arguments (JSON)</label>
        <textarea
          class="mcp-args-input"
          bind:value={toolArgs}
          rows="6"
          spellcheck="false"
        ></textarea>
        <button class="mcp-call-btn" onclick={callTool} disabled={loading}>
          {#if loading}
            Calling...
          {:else}
            Call Tool
          {/if}
        </button>
      </div>

      {#if toolResult}
        <div class="mcp-result" class:mcp-result-error={toolResult.is_error}>
          <div class="mcp-result-title">{toolResult.is_error ? 'Error' : 'Result'}</div>
          {#each toolResult.content as item}
            {#if item.text}
              <pre class="mcp-result-text">{item.text}</pre>
            {:else}
              <pre class="mcp-result-text">{JSON.stringify(item, null, 2)}</pre>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .mcp-explorer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    font-size: var(--font-size-sm);
  }

  .mcp-breadcrumb {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-primary);
  }

  .mcp-bc-link {
    background: none;
    border: none;
    color: var(--accent-primary);
    cursor: pointer;
    padding: 0;
    font-size: inherit;
    font-family: inherit;
  }
  .mcp-bc-link:hover { text-decoration: underline; }
  .mcp-bc-sep { color: var(--text-muted); opacity: 0.5; }
  .mcp-bc-active { color: var(--text-primary); }

  .mcp-error {
    background: rgba(255, 80, 80, 0.1);
    border: 1px solid rgba(255, 80, 80, 0.3);
    color: #ff5050;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 12px;
    word-break: break-word;
  }

  .mcp-section { display: flex; flex-direction: column; gap: 6px; }

  .mcp-section-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .mcp-empty {
    color: var(--text-muted);
    font-size: 12px;
    padding: 16px 0;
    text-align: center;
    line-height: 1.5;
  }
  .mcp-loading {
    color: var(--text-muted);
    font-size: 12px;
    padding: 12px 0;
    font-style: italic;
  }

  .mcp-server-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 10px 12px;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    transition: border-color 0.15s;
  }
  .mcp-server-item:hover { border-color: var(--accent-primary); }
  .mcp-server-item:disabled { opacity: 0.5; cursor: wait; }

  .mcp-server-name {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-primary);
    font-weight: 500;
    font-size: 13px;
  }

  .mcp-server-url {
    color: var(--text-muted);
    font-size: 11px;
    font-family: var(--font-mono);
    padding-left: 20px;
  }

  .mcp-server-info {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .mcp-info-badge {
    background: rgba(80, 250, 123, 0.15);
    color: #50fa7b;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .mcp-info-detail {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .mcp-muted { color: var(--text-muted); }

  .mcp-tool-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 8px 10px;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    transition: border-color 0.15s;
  }
  .mcp-tool-item:hover { border-color: var(--accent-primary); }

  .mcp-tool-name {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-primary);
    font-weight: 500;
    font-size: 12px;
    font-family: var(--font-mono);
  }

  .mcp-tool-desc {
    color: var(--text-muted);
    font-size: 11px;
    padding-left: 18px;
    line-height: 1.4;
  }

  .mcp-tool-full-desc {
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.5;
    margin-bottom: 4px;
  }

  .mcp-schema-info {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .mcp-schema-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .mcp-schema-param {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: 12px;
    flex-wrap: wrap;
  }
  .mcp-schema-param code {
    color: var(--accent-primary);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .mcp-schema-type {
    color: var(--text-muted);
    font-size: 10px;
    font-family: var(--font-mono);
    background: var(--bg-tertiary);
    padding: 1px 5px;
    border-radius: 3px;
  }
  .mcp-schema-required {
    color: #ff6b6b;
    font-size: 10px;
    font-style: italic;
  }
  .mcp-schema-desc {
    color: var(--text-muted);
    font-size: 11px;
  }

  .mcp-call-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
  }

  .mcp-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .mcp-args-input {
    background: var(--bg-primary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 10px;
    resize: vertical;
    min-height: 80px;
    outline: none;
  }
  .mcp-args-input:focus { border-color: var(--border-focus); }

  .mcp-call-btn {
    align-self: flex-start;
    background: var(--accent-primary);
    color: var(--bg-primary);
    border: none;
    border-radius: 6px;
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    font-family: inherit;
    transition: opacity 0.15s;
  }
  .mcp-call-btn:hover { opacity: 0.85; }
  .mcp-call-btn:disabled { opacity: 0.5; cursor: wait; }

  .mcp-result {
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    padding: 10px 12px;
    margin-top: 8px;
  }

  .mcp-result-error {
    border-color: rgba(255, 80, 80, 0.3);
  }

  .mcp-result-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 6px;
  }

  .mcp-result-error .mcp-result-title { color: #ff5050; }

  .mcp-result-text {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-primary);
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    line-height: 1.5;
    max-height: 400px;
    overflow-y: auto;
  }
</style>
