<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface PingReply {
    seq: number;
    ttl: number;
    time_ms: number;
  }

  interface PingStats {
    transmitted: number;
    received: number;
    loss_percent: number;
    min_ms: number;
    avg_ms: number;
    max_ms: number;
    stddev_ms: number;
  }

  interface PingResult {
    host: string;
    ip: string;
    replies: PingReply[];
    stats: PingStats;
  }

  let host = $state('');
  let count = $state('5');
  let result: PingResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');

  async function runPing() {
    if (!host.trim()) return;
    loading = true;
    error = '';
    result = null;
    try {
      const c = parseInt(count, 10);
      result = await invoke<PingResult>('run_ping', {
        host: host.trim(),
        count: isNaN(c) ? null : c,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') runPing();
  }

  function latencyColor(ms: number, max: number): string {
    if (max === 0) return '#50fa7b';
    const ratio = ms / max;
    if (ratio < 0.4) return '#50fa7b';
    if (ratio < 0.7) return '#f1fa8c';
    return '#ffb86c';
  }

  function lossColor(pct: number): string {
    if (pct === 0) return '#50fa7b';
    if (pct < 20) return '#f1fa8c';
    if (pct < 50) return '#ffb86c';
    return '#ff5555';
  }

  function barWidth(ms: number, max: number): number {
    if (max === 0) return 0;
    return Math.max(4, Math.round((ms / max) * 100));
  }
</script>

<div class="ping-app">
  <div class="ping-header">
    <div class="ping-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline></svg>
      <span>Ping</span>
    </div>
  </div>

  <div class="ping-input-row">
    <input
      class="ping-input ping-host"
      type="text"
      bind:value={host}
      onkeydown={handleKeydown}
      placeholder="example.com"
      spellcheck="false"
      autocomplete="off"
    />
    <input
      class="ping-input ping-count"
      type="text"
      bind:value={count}
      onkeydown={handleKeydown}
      placeholder="5"
      spellcheck="false"
    />
    <button class="ping-btn" onclick={runPing} disabled={loading || !host.trim()}>
      {#if loading}
        <svg class="ping-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        Pinging...
      {:else}
        Ping
      {/if}
    </button>
  </div>

  {#if error}
    <div class="ping-error">{error}</div>
  {/if}

  {#if result}
    <div class="ping-results">
      <!-- Stats summary -->
      <div class="ping-stats-bar">
        <div class="ping-stat">
          <span class="ping-stat-label">Host</span>
          <span class="ping-stat-value">{result.ip}</span>
        </div>
        <div class="ping-stat">
          <span class="ping-stat-label">Loss</span>
          <span class="ping-stat-value" style="color: {lossColor(result.stats.loss_percent)}">{result.stats.loss_percent}%</span>
        </div>
        <div class="ping-stat">
          <span class="ping-stat-label">Min</span>
          <span class="ping-stat-value">{result.stats.min_ms.toFixed(1)}ms</span>
        </div>
        <div class="ping-stat">
          <span class="ping-stat-label">Avg</span>
          <span class="ping-stat-value">{result.stats.avg_ms.toFixed(1)}ms</span>
        </div>
        <div class="ping-stat">
          <span class="ping-stat-label">Max</span>
          <span class="ping-stat-value">{result.stats.max_ms.toFixed(1)}ms</span>
        </div>
      </div>

      <!-- Reply bars -->
      <div class="ping-replies">
        {#each result.replies as reply}
          <div class="ping-reply">
            <span class="ping-reply-seq">#{reply.seq}</span>
            <div class="ping-reply-bar-wrap">
              <div
                class="ping-reply-bar"
                style="width: {barWidth(reply.time_ms, result.stats.max_ms)}%; background: {latencyColor(reply.time_ms, result.stats.max_ms)}"
              ></div>
            </div>
            <span class="ping-reply-time">{reply.time_ms.toFixed(1)}ms</span>
            <span class="ping-reply-ttl">TTL {reply.ttl}</span>
          </div>
        {/each}
      </div>

      <!-- Packet info -->
      <div class="ping-packet-info">
        {result.stats.received}/{result.stats.transmitted} packets received
        {#if result.stats.stddev_ms > 0}
          &middot; stddev {result.stats.stddev_ms.toFixed(2)}ms
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .ping-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .ping-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ping-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #f1fa8c;
    font-weight: 600;
    font-size: 13px;
  }

  .ping-logo svg { color: #f1fa8c; }

  .ping-input-row {
    display: flex;
    gap: 6px;
  }

  .ping-input {
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

  .ping-input:focus { border-color: #3a3a5a; }
  .ping-input::placeholder { color: #4a4a5e; }

  .ping-host { flex: 1; min-width: 0; }
  .ping-count { width: 48px; text-align: center; }

  .ping-btn {
    background: #1e1e2e;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #f1fa8c;
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

  .ping-btn:hover:not(:disabled) { background: #2a2a3e; }
  .ping-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .ping-spinner { animation: ping-spin 1s linear infinite; }
  @keyframes ping-spin { to { transform: rotate(360deg); } }

  .ping-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  .ping-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ping-stats-bar {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    padding: 8px 10px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
  }

  .ping-stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ping-stat-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
  }

  .ping-stat-value {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .ping-replies {
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    padding: 8px;
  }

  .ping-reply {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ping-reply-seq {
    font-size: 10px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 20px;
  }

  .ping-reply-bar-wrap {
    flex: 1;
    height: 8px;
    background: #1a1a28;
    border-radius: 4px;
    overflow: hidden;
  }

  .ping-reply-bar {
    height: 100%;
    border-radius: 4px;
    min-width: 4px;
    transition: width 0.3s ease;
  }

  .ping-reply-time {
    font-size: 10px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 55px;
    text-align: right;
  }

  .ping-reply-ttl {
    font-size: 9px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 40px;
  }

  .ping-packet-info {
    font-size: 10px;
    color: #6e6e82;
    text-align: center;
    padding: 2px;
  }
</style>
