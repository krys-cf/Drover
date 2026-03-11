<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface DnsRecord {
    name: string;
    ttl: string;
    record_class: string;
    record_type: string;
    value: string;
  }

  interface DigResult {
    domain: string;
    records: DnsRecord[];
    query_time: string;
    server: string;
    status: string;
  }

  interface TraceRecord {
    name: string;
    ttl: string;
    record_class: string;
    record_type: string;
    value: string;
  }

  interface TraceHop {
    zone: string;
    server: string;
    server_ip: string;
    response_time: string;
    response_bytes: string;
    records: TraceRecord[];
  }

  interface DigTraceResult {
    domain: string;
    hops: TraceHop[];
    final_answers: TraceRecord[];
  }

  let domain = $state('');
  let recordType = $state('ANY');
  let mode = $state<'lookup' | 'trace'>('lookup');
  let result: DigResult | null = $state(null);
  let traceResult: DigTraceResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');
  let includeDnssec = $state(false);

  const RECORD_TYPES = ['ANY', 'A', 'AAAA', 'CNAME', 'MX', 'NS', 'TXT', 'SOA', 'SRV', 'CAA', 'PTR'];

  const TYPE_LABELS: Record<string, { label: string; description: string; color: string }> = {
    A:     { label: 'IPv4 Address',     description: 'Points the domain to a server IP address',           color: '#8be9fd' },
    AAAA:  { label: 'IPv6 Address',     description: 'Points the domain to an IPv6 server address',        color: '#8be9fd' },
    CNAME: { label: 'Alias',            description: 'Redirects this name to another domain name',         color: '#bd93f9' },
    MX:    { label: 'Mail Server',      description: 'Handles email delivery for this domain',             color: '#ffb86c' },
    NS:    { label: 'Name Server',      description: 'Authoritative DNS server for this domain',           color: '#50fa7b' },
    TXT:   { label: 'Text Record',      description: 'Verification, SPF, DKIM, or other text data',       color: '#f1fa8c' },
    SOA:   { label: 'Start of Authority', description: 'Primary DNS info and zone transfer settings',      color: '#ff79c6' },
    SRV:   { label: 'Service',          description: 'Defines a service location (host + port)',           color: '#ff5555' },
    CAA:   { label: 'Certificate Auth', description: 'Which certificate authorities can issue SSL certs',  color: '#f8f8f2' },
    PTR:   { label: 'Pointer',          description: 'Reverse DNS — maps an IP back to a domain name',    color: '#6272a4' },
  };

  const HOP_LABELS: Record<number, { label: string; icon: string; color: string }> = {
    0: { label: 'Root DNS', icon: '/', color: '#ff79c6' },
    1: { label: 'TLD Server', icon: '.', color: '#ffb86c' },
    2: { label: 'Authoritative NS', icon: '@', color: '#50fa7b' },
    3: { label: 'Final Answer', icon: '=', color: '#8be9fd' },
  };

  function getHopLabel(index: number, totalHops: number) {
    if (index === 0) return HOP_LABELS[0];
    if (index === totalHops - 1) return { label: 'Final Answer', icon: '=', color: '#8be9fd' };
    if (index === 1) return HOP_LABELS[1];
    return { label: 'Authoritative NS', icon: '@', color: '#50fa7b' };
  }

  function getTypeInfo(type: string) {
    return TYPE_LABELS[type] || { label: type, description: 'DNS record', color: '#e4e4ed' };
  }

  function formatTTL(ttl: string): string {
    const seconds = parseInt(ttl, 10);
    if (isNaN(seconds)) return ttl;
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
    return `${Math.floor(seconds / 86400)}d`;
  }

  function friendlyValue(record: DnsRecord | TraceRecord): string {
    if (record.record_type === 'MX') {
      const parts = record.value.split(/\s+/);
      if (parts.length >= 2) return `${parts[1]} (priority ${parts[0]})`;
    }
    if (record.record_type === 'SOA') {
      const parts = record.value.split(/\s+/);
      if (parts.length >= 2) return `Primary: ${parts[0]}  Contact: ${parts[1]}`;
    }
    if (record.record_type === 'TXT') {
      return record.value.replace(/^"|"$/g, '');
    }
    return record.value;
  }

  function friendlyZone(zone: string): string {
    if (zone === '.') return 'Root (.)';
    return zone.replace(/\.$/, '');
  }

  async function runDig() {
    if (!domain.trim()) return;
    loading = true;
    error = '';
    result = null;
    traceResult = null;

    try {
      if (mode === 'trace') {
        traceResult = await invoke<DigTraceResult>('dig_trace', {
          domain: domain.trim(),
          includeDnssec,
        });
      } else {
        result = await invoke<DigResult>('dig_domain', {
          domain: domain.trim(),
          recordType: recordType === 'ANY' ? null : recordType,
        });
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') runDig();
  }

  function groupByType(records: DnsRecord[]): Map<string, DnsRecord[]> {
    const map = new Map<string, DnsRecord[]>();
    for (const r of records) {
      const existing = map.get(r.record_type) || [];
      existing.push(r);
      map.set(r.record_type, existing);
    }
    return map;
  }

  function totalTraceTime(hops: TraceHop[]): string {
    let total = 0;
    for (const hop of hops) {
      const ms = parseInt(hop.response_time, 10);
      if (!isNaN(ms)) total += ms;
    }
    return `${total} ms`;
  }
</script>

<div class="dig-app">
  <div class="dig-header">
    <div class="dig-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
      <span>Dig</span>
    </div>
    <div class="dig-mode-toggle">
      <button class="dig-mode-btn" class:active={mode === 'lookup'} onclick={() => { mode = 'lookup'; result = null; traceResult = null; error = ''; }}>Lookup</button>
      <button class="dig-mode-btn" class:active={mode === 'trace'} onclick={() => { mode = 'trace'; result = null; traceResult = null; error = ''; }}>Trace</button>
    </div>
  </div>

  <div class="dig-input-row">
    <input
      class="dig-input"
      type="text"
      bind:value={domain}
      onkeydown={handleKeydown}
      placeholder="example.com"
      spellcheck="false"
      autocomplete="off"
    />
    {#if mode === 'lookup'}
      <select class="dig-select" bind:value={recordType}>
        {#each RECORD_TYPES as rt}
          <option value={rt}>{rt}</option>
        {/each}
      </select>
    {/if}
    <button class="dig-btn" onclick={runDig} disabled={loading || !domain.trim()}>
      {#if loading}
        <svg class="dig-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        {mode === 'trace' ? 'Tracing...' : 'Looking up...'}
      {:else}
        {mode === 'trace' ? 'Trace' : 'Dig'}
      {/if}
    </button>
  </div>

  {#if mode === 'trace'}
    <div class="dig-trace-hint">
      Traces the full DNS delegation chain from root servers to the final answer
    </div>
  {/if}

  {#if error}
    <div class="dig-error">{error}</div>
  {/if}

  <!-- Lookup results -->
  {#if result}
    <div class="dig-results">
      <div class="dig-meta">
        <span class="dig-meta-item">
          <span class="dig-meta-label">Status</span>
          <span class="dig-meta-value" class:dig-status-ok={result.status === 'NOERROR'} class:dig-status-err={result.status !== 'NOERROR'}>{result.status}</span>
        </span>
        {#if result.query_time}
          <span class="dig-meta-item">
            <span class="dig-meta-label">Query Time</span>
            <span class="dig-meta-value">{result.query_time}</span>
          </span>
        {/if}
        {#if result.server}
          <span class="dig-meta-item">
            <span class="dig-meta-label">Server</span>
            <span class="dig-meta-value">{result.server}</span>
          </span>
        {/if}
        <span class="dig-meta-item">
          <span class="dig-meta-label">Records</span>
          <span class="dig-meta-value">{result.records.length}</span>
        </span>
      </div>

      {#if result.records.length === 0}
        <div class="dig-empty">No records found for <strong>{result.domain}</strong></div>
      {:else}
        {@const grouped = groupByType(result.records)}
        {#each [...grouped.entries()] as [type, records]}
          {@const info = getTypeInfo(type)}
          <div class="dig-group">
            <div class="dig-group-header">
              <span class="dig-type-badge" style="background: {info.color}20; color: {info.color}; border-color: {info.color}40">{type}</span>
              <span class="dig-type-label">{info.label}</span>
              <span class="dig-type-desc">{info.description}</span>
            </div>
            <div class="dig-records">
              {#each records as record}
                <div class="dig-record">
                  <div class="dig-record-value">{friendlyValue(record)}</div>
                  <div class="dig-record-meta">
                    <span class="dig-record-ttl" title="Time to live">TTL {formatTTL(record.ttl)}</span>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {/if}

  <!-- Trace results -->
  {#if traceResult}
    <div class="dig-results">
      <div class="dig-meta">
        <span class="dig-meta-item">
          <span class="dig-meta-label">Domain</span>
          <span class="dig-meta-value">{traceResult.domain}</span>
        </span>
        <span class="dig-meta-item">
          <span class="dig-meta-label">Hops</span>
          <span class="dig-meta-value">{traceResult.hops.length}</span>
        </span>
        <span class="dig-meta-item">
          <span class="dig-meta-label">Total Time</span>
          <span class="dig-meta-value">{totalTraceTime(traceResult.hops)}</span>
        </span>
        {#if traceResult.final_answers.length > 0}
          <span class="dig-meta-item">
            <span class="dig-meta-label">Answer</span>
            <span class="dig-meta-value dig-status-ok">{traceResult.final_answers.map(r => r.value).join(', ')}</span>
          </span>
        {/if}
      </div>

      <div class="trace-chain">
        {#each traceResult.hops as hop, i}
          {@const hopLabel = getHopLabel(i, traceResult.hops.length)}
          <div class="trace-hop" class:trace-hop-final={i === traceResult.hops.length - 1}>
            <div class="trace-hop-connector">
              <div class="trace-hop-dot" style="background: {hopLabel.color}; box-shadow: 0 0 6px {hopLabel.color}40"></div>
              {#if i < traceResult.hops.length - 1}
                <div class="trace-hop-line"></div>
              {/if}
            </div>
            <div class="trace-hop-card">
              <div class="trace-hop-header">
                <span class="trace-hop-badge" style="background: {hopLabel.color}18; color: {hopLabel.color}; border-color: {hopLabel.color}30">{hopLabel.label}</span>
                <span class="trace-hop-zone">{friendlyZone(hop.zone)}</span>
                <span class="trace-hop-time">{hop.response_time}</span>
              </div>
              <div class="trace-hop-server">
                <svg viewBox="0 0 24 24" width="11" height="11" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
                <span>{hop.server}</span>
                {#if hop.server !== hop.server_ip}
                  <span class="trace-hop-ip">{hop.server_ip}</span>
                {/if}
              </div>
              <div class="trace-hop-records">
                {#each hop.records as record}
                  {@const info = getTypeInfo(record.record_type)}
                  <div class="trace-record">
                    <span class="trace-record-type" style="color: {info.color}">{record.record_type}</span>
                    <span class="trace-record-value">{friendlyValue(record)}</span>
                    <span class="trace-record-ttl">{formatTTL(record.ttl)}</span>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        {/each}
      </div>

      {#if traceResult.hops.length === 0}
        <div class="dig-empty">No trace hops returned for <strong>{traceResult.domain}</strong></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .dig-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dig-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .dig-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #8be9fd;
    font-weight: 600;
    font-size: 13px;
  }

  .dig-logo svg { color: #8be9fd; }

  .dig-mode-toggle {
    display: flex;
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    overflow: hidden;
  }

  .dig-mode-btn {
    padding: 4px 12px;
    font-size: 11px;
    font-family: inherit;
    color: #6e6e82;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 0.15s;
  }

  .dig-mode-btn.active {
    background: #1e2a3a;
    color: #8be9fd;
  }

  .dig-mode-btn:hover:not(.active) {
    color: #b4b4c4;
  }

  .dig-trace-hint {
    font-size: 10px;
    color: #6e6e82;
    line-height: 1.4;
    padding: 0 2px;
  }

  .dig-input-row {
    display: flex;
    gap: 6px;
  }

  .dig-input {
    flex: 1;
    min-width: 0;
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 10px;
    color: #e4e4ed;
    font-size: 12px;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s;
  }

  .dig-input:focus { border-color: #3a3a5a; }
  .dig-input::placeholder { color: #4a4a5e; }

  .dig-select {
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 8px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: inherit;
    outline: none;
    cursor: pointer;
    min-width: 58px;
  }

  .dig-select:focus { border-color: #3a3a5a; }

  .dig-btn {
    background: #1e1e2e;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 12px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    transition: background 0.15s, border-color 0.15s;
  }

  .dig-btn:hover:not(:disabled) {
    background: #2a2a3e;
    border-color: #3a3a5a;
  }

  .dig-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .dig-spinner {
    animation: dig-spin 1s linear infinite;
  }

  @keyframes dig-spin {
    to { transform: rotate(360deg); }
  }

  .dig-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  .dig-results {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dig-meta {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
    padding: 8px 10px;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
  }

  .dig-meta-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dig-meta-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
  }

  .dig-meta-value {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .dig-status-ok { color: #50fa7b; }
  .dig-status-err { color: #ff5555; }

  .dig-empty {
    text-align: center;
    color: #6e6e82;
    font-size: 12px;
    padding: 20px;
  }

  .dig-group {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    overflow: hidden;
  }

  .dig-group-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid #1e1e2e;
    flex-wrap: wrap;
  }

  .dig-type-badge {
    font-size: 10px;
    font-weight: 700;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid;
    letter-spacing: 0.5px;
  }

  .dig-type-label {
    font-size: 12px;
    font-weight: 600;
    color: #e4e4ed;
  }

  .dig-type-desc {
    font-size: 10px;
    color: #6e6e82;
    flex-basis: 100%;
  }

  .dig-records {
    display: flex;
    flex-direction: column;
  }

  .dig-record {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid #1a1a28;
    gap: 8px;
  }

  .dig-record:last-child { border-bottom: none; }

  .dig-record-value {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    word-break: break-all;
    flex: 1;
    min-width: 0;
  }

  .dig-record-meta {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .dig-record-ttl {
    font-size: 10px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  /* ── Trace chain ── */
  .trace-chain {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding: 4px 0;
  }

  .trace-hop {
    display: flex;
    gap: 12px;
    position: relative;
  }

  .trace-hop-connector {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 16px;
    flex-shrink: 0;
    padding-top: 12px;
  }

  .trace-hop-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    z-index: 1;
  }

  .trace-hop-line {
    width: 2px;
    flex: 1;
    background: #2a2a3a;
    margin-top: 2px;
  }

  .trace-hop-card {
    flex: 1;
    min-width: 0;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    margin-bottom: 8px;
    overflow: hidden;
    transition: border-color 0.15s;
  }

  .trace-hop-final .trace-hop-card {
    border-color: #8be9fd30;
    background: #0d1520;
  }

  .trace-hop-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid #1a1a28;
  }

  .trace-hop-badge {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid;
    white-space: nowrap;
  }

  .trace-hop-zone {
    font-size: 12px;
    font-weight: 600;
    color: #e4e4ed;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .trace-hop-time {
    font-size: 10px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    white-space: nowrap;
  }

  .trace-hop-server {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    font-size: 10px;
    color: #8888a0;
    border-bottom: 1px solid #1a1a28;
  }

  .trace-hop-server svg { color: #6e6e82; flex-shrink: 0; }

  .trace-hop-ip {
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    font-size: 10px;
  }

  .trace-hop-records {
    display: flex;
    flex-direction: column;
  }

  .trace-record {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid #14141e;
  }

  .trace-record:last-child { border-bottom: none; }

  .trace-record-type {
    font-size: 10px;
    font-weight: 700;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 32px;
    letter-spacing: 0.3px;
  }

  .trace-record-value {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .trace-record-ttl {
    font-size: 9px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    flex-shrink: 0;
  }
</style>
