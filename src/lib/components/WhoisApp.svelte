<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface WhoisField {
    key: string;
    value: string;
  }

  interface WhoisResult {
    domain: string;
    registrar: string;
    creation_date: string;
    expiry_date: string;
    updated_date: string;
    nameservers: string[];
    status: string[];
    fields: WhoisField[];
    raw_output: string;
  }

  let domain = $state('');
  let result: WhoisResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');
  let tab = $state<'overview' | 'raw'>('overview');

  async function runWhois() {
    if (!domain.trim()) return;
    loading = true;
    error = '';
    result = null;
    try {
      result = await invoke<WhoisResult>('run_whois', { domain: domain.trim() });
      tab = 'overview';
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') runWhois();
  }

  function friendlyStatus(s: string): string {
    if (s.startsWith('client')) return s.replace('client', '').replace(/([A-Z])/g, ' $1').trim();
    return s;
  }
</script>

<div class="whois-app">
  <div class="whois-header">
    <div class="whois-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"></path><circle cx="12" cy="10" r="3"></circle></svg>
      <span>Whois</span>
    </div>
  </div>

  <div class="whois-input-row">
    <input
      class="whois-input"
      type="text"
      bind:value={domain}
      onkeydown={handleKeydown}
      placeholder="example.com"
      spellcheck="false"
      autocomplete="off"
    />
    <button class="whois-btn" onclick={runWhois} disabled={loading || !domain.trim()}>
      {#if loading}
        <svg class="whois-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        Looking up...
      {:else}
        Lookup
      {/if}
    </button>
  </div>

  {#if error}
    <div class="whois-error">{error}</div>
  {/if}

  {#if result}
    <div class="whois-results">
      <div class="whois-tabs">
        <button class="whois-tab" class:active={tab === 'overview'} onclick={() => tab = 'overview'}>Overview</button>
        <button class="whois-tab" class:active={tab === 'raw'} onclick={() => tab = 'raw'}>Raw</button>
      </div>

      <div class="whois-tab-content">
        {#if tab === 'overview'}
          <div class="whois-fields">
            {#if result.registrar}
              <div class="whois-field">
                <span class="whois-field-label">Registrar</span>
                <span class="whois-field-value">{result.registrar}</span>
              </div>
            {/if}
            {#if result.creation_date}
              <div class="whois-field">
                <span class="whois-field-label">Created</span>
                <span class="whois-field-value">{result.creation_date}</span>
              </div>
            {/if}
            {#if result.expiry_date}
              <div class="whois-field">
                <span class="whois-field-label">Expires</span>
                <span class="whois-field-value">{result.expiry_date}</span>
              </div>
            {/if}
            {#if result.updated_date}
              <div class="whois-field">
                <span class="whois-field-label">Updated</span>
                <span class="whois-field-value">{result.updated_date}</span>
              </div>
            {/if}
            {#if result.nameservers.length > 0}
              <div class="whois-field">
                <span class="whois-field-label">Nameservers</span>
                <div class="whois-ns-list">
                  {#each result.nameservers as ns}
                    <span class="whois-ns-chip">{ns}</span>
                  {/each}
                </div>
              </div>
            {/if}
            {#if result.status.length > 0}
              <div class="whois-field">
                <span class="whois-field-label">Status</span>
                <div class="whois-status-list">
                  {#each result.status as s}
                    <span class="whois-status-chip">{friendlyStatus(s)}</span>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {:else if tab === 'raw'}
          <pre class="whois-raw">{result.raw_output}</pre>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .whois-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .whois-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .whois-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #bd93f9;
    font-weight: 600;
    font-size: 13px;
  }

  .whois-logo svg { color: #bd93f9; }

  .whois-input-row {
    display: flex;
    gap: 6px;
  }

  .whois-input {
    flex: 1;
    min-width: 0;
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 10px;
    color: #e4e4ed;
    font-size: 12px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
    transition: border-color 0.15s;
  }

  .whois-input:focus { border-color: #3a3a5a; }
  .whois-input::placeholder { color: #4a4a5e; }

  .whois-btn {
    background: #1e1e2e;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #bd93f9;
    font-size: 11px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    transition: background 0.15s;
  }

  .whois-btn:hover:not(:disabled) { background: #2a2a3e; }
  .whois-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .whois-spinner { animation: whois-spin 1s linear infinite; }
  @keyframes whois-spin { to { transform: rotate(360deg); } }

  .whois-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  .whois-results {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .whois-tabs {
    display: flex;
    gap: 0;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px 6px 0 0;
    padding: 0 4px;
    border-bottom: 1px solid #1e1e2e;
  }

  .whois-tab {
    padding: 6px 10px;
    font-size: 11px;
    font-family: inherit;
    color: #6e6e82;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: all 0.15s;
  }

  .whois-tab.active {
    color: #e4e4ed;
    border-bottom-color: #bd93f9;
  }

  .whois-tab:hover:not(.active) { color: #b4b4c4; }

  .whois-tab-content {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-top: none;
    border-radius: 0 0 6px 6px;
    overflow: hidden;
  }

  .whois-fields {
    display: flex;
    flex-direction: column;
  }

  .whois-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 7px 10px;
    border-bottom: 1px solid #14141e;
  }

  .whois-field:last-child { border-bottom: none; }

  .whois-field-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
    font-weight: 600;
  }

  .whois-field-value {
    font-size: 11px;
    color: #e4e4ed;
    word-break: break-all;
  }

  .whois-ns-list, .whois-status-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .whois-ns-chip {
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    padding: 2px 6px;
    background: #50fa7b12;
    color: #50fa7b;
    border: 1px solid #50fa7b25;
    border-radius: 4px;
  }

  .whois-status-chip {
    font-size: 10px;
    padding: 2px 6px;
    background: #8be9fd12;
    color: #8be9fd;
    border: 1px solid #8be9fd25;
    border-radius: 4px;
  }

  .whois-raw {
    padding: 10px;
    margin: 0;
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #8888a0;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 500px;
    overflow-y: auto;
    line-height: 1.5;
  }
</style>
