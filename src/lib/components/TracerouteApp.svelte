<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface TracerouteHop {
    hop: number;
    host: string;
    ip: string;
    times_ms: number[];
    is_timeout: boolean;
  }

  interface TracerouteResult {
    target: string;
    target_ip: string;
    max_hops: number;
    hops: TracerouteHop[];
  }

  let host = $state('');
  let maxHops = $state('15');
  let result: TracerouteResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');

  async function runTrace() {
    if (!host.trim()) return;
    loading = true;
    error = '';
    result = null;
    try {
      const mh = parseInt(maxHops, 10);
      result = await invoke<TracerouteResult>('run_traceroute', {
        host: host.trim(),
        maxHops: isNaN(mh) ? null : mh,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') runTrace();
  }

  function avgTime(times: number[]): number {
    if (times.length === 0) return 0;
    return times.reduce((a, b) => a + b, 0) / times.length;
  }

  function maxTime(hops: TracerouteHop[]): number {
    let max = 0;
    for (const hop of hops) {
      for (const t of hop.times_ms) {
        if (t > max) max = t;
      }
    }
    return max;
  }

  function hopBarWidth(avg: number, max: number): number {
    if (max === 0) return 0;
    return Math.max(4, Math.round((avg / max) * 100));
  }

  function hopColor(avg: number, max: number): string {
    if (max === 0) return '#50fa7b';
    const ratio = avg / max;
    if (ratio < 0.3) return '#50fa7b';
    if (ratio < 0.6) return '#f1fa8c';
    if (ratio < 0.8) return '#ffb86c';
    return '#ff5555';
  }
</script>

<div class="trace-app">
  <div class="trace-header">
    <div class="trace-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path></svg>
      <span>Traceroute</span>
    </div>
  </div>

  <div class="trace-input-row">
    <input
      class="trace-input trace-host"
      type="text"
      bind:value={host}
      onkeydown={handleKeydown}
      placeholder="example.com"
      spellcheck="false"
      autocomplete="off"
    />
    <input
      class="trace-input trace-hops"
      type="text"
      bind:value={maxHops}
      onkeydown={handleKeydown}
      placeholder="20"
      spellcheck="false"
      title="Max hops"
    />
    <button class="trace-btn" onclick={runTrace} disabled={loading || !host.trim()}>
      {#if loading}
        <svg class="trace-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        Tracing...
      {:else}
        Trace
      {/if}
    </button>
  </div>

  {#if error}
    <div class="trace-error">{error}</div>
  {/if}

  {#if result}
    <div class="trace-results">
      <div class="trace-meta">
        <span class="trace-meta-item">
          <span class="trace-meta-label">Target</span>
          <span class="trace-meta-value">{result.target_ip || result.target}</span>
        </span>
        <span class="trace-meta-item">
          <span class="trace-meta-label">Hops</span>
          <span class="trace-meta-value">{result.hops.length}</span>
        </span>
      </div>

      <div class="trace-hops">
        {#each result.hops as hop}
          {@const avg = avgTime(hop.times_ms)}
          {@const globalMax = maxTime(result.hops)}
          <div class="trace-hop" class:trace-hop-timeout={hop.is_timeout}>
            <span class="trace-hop-num">{hop.hop}</span>
            {#if hop.is_timeout}
              <div class="trace-hop-info">
                <span class="trace-hop-host trace-timeout">* * *</span>
              </div>
              <span class="trace-hop-time trace-timeout">timeout</span>
            {:else}
              <div class="trace-hop-info">
                <span class="trace-hop-host">{hop.host}</span>
                {#if hop.ip && hop.ip !== hop.host}
                  <span class="trace-hop-ip">{hop.ip}</span>
                {/if}
              </div>
              <div class="trace-hop-bar-wrap">
                <div
                  class="trace-hop-bar"
                  style="width: {hopBarWidth(avg, globalMax)}%; background: {hopColor(avg, globalMax)}"
                ></div>
              </div>
              <span class="trace-hop-time">{avg.toFixed(1)}ms</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .trace-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .trace-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .trace-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #ff79c6;
    font-weight: 600;
    font-size: 13px;
  }

  .trace-logo svg { color: #ff79c6; }

  .trace-input-row {
    display: flex;
    gap: 6px;
  }

  .trace-input {
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

  .trace-input:focus { border-color: #3a3a5a; }
  .trace-input::placeholder { color: #4a4a5e; }

  .trace-host { flex: 1; min-width: 0; }
  .trace-hops { width: 48px; text-align: center; }

  .trace-btn {
    background: #1e1e2e;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #ff79c6;
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

  .trace-btn:hover:not(:disabled) { background: #2a2a3e; }
  .trace-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .trace-spinner { animation: trace-spin 1s linear infinite; }
  @keyframes trace-spin { to { transform: rotate(360deg); } }

  .trace-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  .trace-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .trace-meta {
    display: flex;
    gap: 12px;
    padding: 8px 10px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
  }

  .trace-meta-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .trace-meta-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
  }

  .trace-meta-value {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .trace-hops {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    padding: 6px 8px;
  }

  .trace-hop {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 2px;
  }

  .trace-hop-num {
    font-size: 10px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 18px;
    text-align: right;
  }

  .trace-hop-info {
    display: flex;
    flex-direction: column;
    min-width: 100px;
    flex-shrink: 0;
  }

  .trace-hop-host {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .trace-hop-ip {
    font-size: 9px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .trace-timeout {
    color: #6e6e82;
    font-style: italic;
  }

  .trace-hop-bar-wrap {
    flex: 1;
    height: 6px;
    background: #1a1a28;
    border-radius: 3px;
    overflow: hidden;
  }

  .trace-hop-bar {
    height: 100%;
    border-radius: 3px;
    min-width: 4px;
    transition: width 0.3s ease;
  }

  .trace-hop-time {
    font-size: 10px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 50px;
    text-align: right;
    flex-shrink: 0;
  }

</style>
