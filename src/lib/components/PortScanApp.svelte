<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface PortScanEntry {
    port: number;
    service: string;
    is_open: boolean;
  }

  interface PortScanResult {
    host: string;
    ports: PortScanEntry[];
    open_count: number;
    closed_count: number;
  }

  let host = $state('');
  let customPorts = $state('');
  let result: PortScanResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');

  const COMMON_PRESETS = [
    { label: 'Web', ports: [80, 443, 8080, 8443] },
    { label: 'Mail', ports: [25, 465, 587, 993, 995, 143, 110] },
    { label: 'Database', ports: [3306, 5432, 27017, 6379] },
    { label: 'Remote', ports: [22, 23, 3389, 5900] },
    { label: 'DNS', ports: [53] },
    { label: 'FTP', ports: [21] },
  ];

  let selectedPresets = $state<Set<string>>(new Set(['Web']));

  function togglePreset(label: string) {
    const next = new Set(selectedPresets);
    if (next.has(label)) {
      next.delete(label);
    } else {
      next.add(label);
    }
    selectedPresets = next;
  }

  function getPortList(): number[] {
    const ports = new Set<number>();

    for (const preset of COMMON_PRESETS) {
      if (selectedPresets.has(preset.label)) {
        for (const p of preset.ports) ports.add(p);
      }
    }

    if (customPorts.trim()) {
      for (const part of customPorts.split(',')) {
        const trimmed = part.trim();
        if (trimmed.includes('-')) {
          const [start, end] = trimmed.split('-').map(s => parseInt(s.trim(), 10));
          if (!isNaN(start) && !isNaN(end) && start <= end && end - start < 1000) {
            for (let i = start; i <= end; i++) ports.add(i);
          }
        } else {
          const p = parseInt(trimmed, 10);
          if (!isNaN(p) && p > 0 && p <= 65535) ports.add(p);
        }
      }
    }

    return Array.from(ports).sort((a, b) => a - b);
  }

  async function runScan() {
    if (!host.trim()) return;
    const ports = getPortList();
    if (ports.length === 0) { error = 'No ports selected'; return; }

    loading = true;
    error = '';
    result = null;
    try {
      result = await invoke<PortScanResult>('run_port_scan', {
        host: host.trim(),
        ports,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') runScan();
  }
</script>

<div class="scan-app">
  <div class="scan-header">
    <div class="scan-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
      <span>Port Scanner</span>
    </div>
  </div>

  <div class="scan-input-row">
    <input
      class="scan-input scan-host"
      type="text"
      bind:value={host}
      onkeydown={handleKeydown}
      placeholder="example.com"
      spellcheck="false"
      autocomplete="off"
    />
    <button class="scan-btn" onclick={runScan} disabled={loading || !host.trim()}>
      {#if loading}
        <svg class="scan-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        Scanning...
      {:else}
        Scan
      {/if}
    </button>
  </div>

  <div class="scan-presets">
    <div class="scan-presets-label">Port Groups</div>
    <div class="scan-preset-chips">
      {#each COMMON_PRESETS as preset}
        <button
          class="scan-preset-chip"
          class:active={selectedPresets.has(preset.label)}
          onclick={() => togglePreset(preset.label)}
          title={preset.ports.join(', ')}
        >{preset.label}</button>
      {/each}
    </div>
  </div>

  <div class="scan-custom">
    <input
      class="scan-input scan-custom-input"
      type="text"
      bind:value={customPorts}
      onkeydown={handleKeydown}
      placeholder="Custom ports: 8000, 9090, 3000-3010"
      spellcheck="false"
    />
  </div>

  {#if error}
    <div class="scan-error">{error}</div>
  {/if}

  {#if result}
    <div class="scan-results">
      <div class="scan-summary">
        <span class="scan-summary-item">
          <span class="scan-summary-count scan-open">{result.open_count}</span>
          <span class="scan-summary-label">open</span>
        </span>
        <span class="scan-summary-item">
          <span class="scan-summary-count scan-closed">{result.closed_count}</span>
          <span class="scan-summary-label">closed</span>
        </span>
        <span class="scan-summary-item">
          <span class="scan-summary-count">{result.ports.length}</span>
          <span class="scan-summary-label">scanned</span>
        </span>
      </div>

      <div class="scan-port-list">
        {#each result.ports as entry}
          <div class="scan-port-row" class:scan-port-open={entry.is_open}>
            <span class="scan-port-indicator" style="background: {entry.is_open ? '#50fa7b' : '#ff5555'}"></span>
            <span class="scan-port-num">{entry.port}</span>
            {#if entry.service}
              <span class="scan-port-service">{entry.service}</span>
            {/if}
            <span class="scan-port-status" style="color: {entry.is_open ? '#50fa7b' : '#ff5555'}">
              {entry.is_open ? 'open' : 'closed'}
            </span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .scan-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .scan-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .scan-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #ff5555;
    font-weight: 600;
    font-size: 13px;
  }

  .scan-logo svg { color: #ff5555; }

  .scan-input-row {
    display: flex;
    gap: 6px;
  }

  .scan-input {
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

  .scan-input:focus { border-color: #3a3a5a; }
  .scan-input::placeholder { color: #4a4a5e; }

  .scan-host { flex: 1; min-width: 0; }
  .scan-custom-input { width: 100%; }

  .scan-btn {
    background: #1e1e2e;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #ff5555;
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

  .scan-btn:hover:not(:disabled) { background: #2a2a3e; }
  .scan-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .scan-spinner { animation: scan-spin 1s linear infinite; }
  @keyframes scan-spin { to { transform: rotate(360deg); } }

  .scan-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  .scan-presets {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .scan-presets-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
    font-weight: 600;
  }

  .scan-preset-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .scan-preset-chip {
    padding: 3px 8px;
    font-size: 10px;
    font-family: inherit;
    color: #6e6e82;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .scan-preset-chip:hover { color: #b4b4c4; border-color: #2a2a3a; }

  .scan-preset-chip.active {
    background: #2a1a1a;
    color: #ff5555;
    border-color: #3a2a2a;
  }

  .scan-custom {
    display: flex;
  }

  .scan-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .scan-summary {
    display: flex;
    gap: 16px;
    padding: 8px 10px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
  }

  .scan-summary-item {
    display: flex;
    align-items: baseline;
    gap: 4px;
  }

  .scan-summary-count {
    font-size: 16px;
    font-weight: 700;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .scan-summary-count.scan-open { color: #50fa7b; }
  .scan-summary-count.scan-closed { color: #ff5555; }

  .scan-summary-label {
    font-size: 10px;
    color: #6e6e82;
  }

  .scan-port-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    overflow: hidden;
  }

  .scan-port-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    background: #111118;
  }

  .scan-port-row:nth-child(even) { background: #0e0e16; }

  .scan-port-indicator {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .scan-port-num {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 45px;
  }

  .scan-port-service {
    font-size: 10px;
    color: #6e6e82;
    flex: 1;
  }

  .scan-port-status {
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }
</style>
