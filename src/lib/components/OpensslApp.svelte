<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface CertChainEntry {
    depth: number;
    subject: string;
    issuer: string;
    key_type: string;
    sig_alg: string;
    not_before: string;
    not_after: string;
  }

  interface CertCheckResult {
    host: string;
    port: number;
    subject: string;
    issuer: string;
    not_before: string;
    not_after: string;
    serial: string;
    fingerprint: string;
    san: string[];
    chain: CertChainEntry[];
    protocol: string;
    cipher: string;
    key_type: string;
    days_remaining: number;
    is_expired: boolean;
    raw_output: string;
  }

  interface CertDecodeResult {
    subject: string;
    issuer: string;
    not_before: string;
    not_after: string;
    serial: string;
    fingerprint: string;
    san: string[];
    key_type: string;
    sig_alg: string;
    version: string;
    raw_text: string;
  }

  interface HashResult {
    input_size: number;
    algorithm: string;
    output: string;
  }

  let mode = $state<'check' | 'decode' | 'hash'>('check');

  // Check cert state
  let checkHost = $state('');
  let checkPort = $state('443');
  let certResult: CertCheckResult | null = $state(null);
  let certLoading = $state(false);
  let certError = $state('');
  let certTab = $state<'overview' | 'chain' | 'raw'>('overview');

  // Decode cert state
  let decodePem = $state('');
  let decodeResult: CertDecodeResult | null = $state(null);
  let decodeLoading = $state(false);
  let decodeError = $state('');
  let decodeTab = $state<'details' | 'raw'>('details');

  // Hash state
  let hashInput = $state('');
  let hashAlgorithm = $state('sha256');
  let hashResult: HashResult | null = $state(null);
  let hashLoading = $state(false);
  let hashError = $state('');

  const HASH_ALGORITHMS = [
    { value: 'sha256', label: 'SHA-256' },
    { value: 'sha1', label: 'SHA-1' },
    { value: 'sha512', label: 'SHA-512' },
    { value: 'md5', label: 'MD5' },
    { value: 'base64-encode', label: 'Base64 Encode' },
    { value: 'base64-decode', label: 'Base64 Decode' },
  ];

  function expiryColor(days: number): string {
    if (days < 0) return '#ff5555';
    if (days < 30) return '#ffb86c';
    if (days < 90) return '#f1fa8c';
    return '#50fa7b';
  }

  function expiryLabel(days: number): string {
    if (days < 0) return `Expired ${Math.abs(days)} days ago`;
    if (days === 0) return 'Expires today';
    if (days === 1) return 'Expires tomorrow';
    return `${days} days remaining`;
  }

  function clearResults() {
    certResult = null;
    certError = '';
    decodeResult = null;
    decodeError = '';
    hashResult = null;
    hashError = '';
  }

  async function checkCert() {
    if (!checkHost.trim()) return;
    certLoading = true;
    certError = '';
    certResult = null;
    try {
      const portNum = parseInt(checkPort, 10);
      certResult = await invoke<CertCheckResult>('openssl_check_cert', {
        host: checkHost.trim(),
        port: isNaN(portNum) ? null : portNum,
      });
      certTab = 'overview';
    } catch (e) {
      certError = String(e);
    } finally {
      certLoading = false;
    }
  }

  async function decodeCert() {
    if (!decodePem.trim()) return;
    decodeLoading = true;
    decodeError = '';
    decodeResult = null;
    try {
      decodeResult = await invoke<CertDecodeResult>('openssl_decode_cert', {
        pem: decodePem.trim(),
      });
      decodeTab = 'details';
    } catch (e) {
      decodeError = String(e);
    } finally {
      decodeLoading = false;
    }
  }

  async function runHash() {
    if (!hashInput.trim()) return;
    hashLoading = true;
    hashError = '';
    hashResult = null;
    try {
      hashResult = await invoke<HashResult>('openssl_hash', {
        text: hashInput,
        algorithm: hashAlgorithm,
      });
    } catch (e) {
      hashError = String(e);
    } finally {
      hashLoading = false;
    }
  }

  function handleCheckKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') checkCert();
  }

  function handleHashKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) runHash();
  }

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
  }
</script>

<div class="ssl-app">
  <div class="ssl-header">
    <div class="ssl-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect><path d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg>
      <span>OpenSSL</span>
    </div>
    <div class="ssl-mode-toggle">
      <button class="ssl-mode-btn" class:active={mode === 'check'} onclick={() => { mode = 'check'; clearResults(); }}>Check Cert</button>
      <button class="ssl-mode-btn" class:active={mode === 'decode'} onclick={() => { mode = 'decode'; clearResults(); }}>Decode</button>
      <button class="ssl-mode-btn" class:active={mode === 'hash'} onclick={() => { mode = 'hash'; clearResults(); }}>Hash</button>
    </div>
  </div>

  <!-- Check Certificate Mode -->
  {#if mode === 'check'}
    <div class="ssl-hint">Connect to a host and inspect its SSL/TLS certificate</div>
    <div class="ssl-input-row">
      <input
        class="ssl-input ssl-host-input"
        type="text"
        bind:value={checkHost}
        onkeydown={handleCheckKeydown}
        placeholder="example.com"
        spellcheck="false"
        autocomplete="off"
      />
      <input
        class="ssl-input ssl-port-input"
        type="text"
        bind:value={checkPort}
        onkeydown={handleCheckKeydown}
        placeholder="443"
        spellcheck="false"
      />
      <button class="ssl-btn" onclick={checkCert} disabled={certLoading || !checkHost.trim()}>
        {#if certLoading}
          <svg class="ssl-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
          Checking...
        {:else}
          Check
        {/if}
      </button>
    </div>

    {#if certError}
      <div class="ssl-error">{certError}</div>
    {/if}

    {#if certResult}
      <div class="ssl-results">
        <!-- Expiry banner -->
        <div class="ssl-expiry-banner" style="background: {expiryColor(certResult.days_remaining)}12; border-color: {expiryColor(certResult.days_remaining)}30">
          <div class="ssl-expiry-icon" style="color: {expiryColor(certResult.days_remaining)}">
            {#if certResult.is_expired}
              <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>
            {:else}
              <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>
            {/if}
          </div>
          <div class="ssl-expiry-info">
            <span class="ssl-expiry-label" style="color: {expiryColor(certResult.days_remaining)}">{expiryLabel(certResult.days_remaining)}</span>
            <span class="ssl-expiry-date">Expires {certResult.not_after}</span>
          </div>
        </div>

        <!-- Tabs -->
        <div class="ssl-tabs">
          <button class="ssl-tab" class:active={certTab === 'overview'} onclick={() => certTab = 'overview'}>Overview</button>
          <button class="ssl-tab" class:active={certTab === 'chain'} onclick={() => certTab = 'chain'}>
            Chain
            <span class="ssl-tab-badge">{certResult.chain.length}</span>
          </button>
          <button class="ssl-tab" class:active={certTab === 'raw'} onclick={() => certTab = 'raw'}>Raw</button>
        </div>

        <div class="ssl-tab-content">
          {#if certTab === 'overview'}
            <div class="ssl-fields">
              <div class="ssl-field">
                <span class="ssl-field-label">Subject</span>
                <span class="ssl-field-value">{certResult.subject}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Issuer</span>
                <span class="ssl-field-value">{certResult.issuer}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Valid From</span>
                <span class="ssl-field-value">{certResult.not_before}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Valid Until</span>
                <span class="ssl-field-value" style="color: {expiryColor(certResult.days_remaining)}">{certResult.not_after}</span>
              </div>
              {#if certResult.san.length > 0}
                <div class="ssl-field">
                  <span class="ssl-field-label">SANs</span>
                  <div class="ssl-san-list">
                    {#each certResult.san as name}
                      <span class="ssl-san-chip">{name}</span>
                    {/each}
                  </div>
                </div>
              {/if}
              <div class="ssl-field">
                <span class="ssl-field-label">Serial</span>
                <span class="ssl-field-value ssl-mono">{certResult.serial}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Fingerprint</span>
                <span class="ssl-field-value ssl-mono ssl-small">{certResult.fingerprint}</span>
              </div>
              {#if certResult.protocol}
                <div class="ssl-field-row">
                  <div class="ssl-field ssl-field-half">
                    <span class="ssl-field-label">Protocol</span>
                    <span class="ssl-field-value">{certResult.protocol}</span>
                  </div>
                  <div class="ssl-field ssl-field-half">
                    <span class="ssl-field-label">Cipher</span>
                    <span class="ssl-field-value ssl-mono ssl-small">{certResult.cipher}</span>
                  </div>
                </div>
              {/if}
              {#if certResult.key_type}
                <div class="ssl-field">
                  <span class="ssl-field-label">Key</span>
                  <span class="ssl-field-value">{certResult.key_type}</span>
                </div>
              {/if}
            </div>
          {:else if certTab === 'chain'}
            <div class="ssl-chain">
              {#each certResult.chain as entry, i}
                <div class="ssl-chain-entry" class:ssl-chain-leaf={i === 0}>
                  <div class="ssl-chain-connector">
                    <div class="ssl-chain-dot" style="background: {i === 0 ? '#8be9fd' : '#6e6e82'}"></div>
                    {#if i < certResult.chain.length - 1}
                      <div class="ssl-chain-line"></div>
                    {/if}
                  </div>
                  <div class="ssl-chain-card">
                    <div class="ssl-chain-card-header">
                      <span class="ssl-chain-depth">Depth {entry.depth}</span>
                      <span class="ssl-chain-subject">{entry.subject}</span>
                    </div>
                    <div class="ssl-chain-card-body">
                      <div class="ssl-chain-detail">
                        <span class="ssl-chain-detail-label">Issuer</span>
                        <span class="ssl-chain-detail-value">{entry.issuer}</span>
                      </div>
                      {#if entry.key_type}
                        <div class="ssl-chain-detail">
                          <span class="ssl-chain-detail-label">Key</span>
                          <span class="ssl-chain-detail-value">{entry.key_type}</span>
                        </div>
                      {/if}
                      {#if entry.sig_alg}
                        <div class="ssl-chain-detail">
                          <span class="ssl-chain-detail-label">Signature</span>
                          <span class="ssl-chain-detail-value">{entry.sig_alg}</span>
                        </div>
                      {/if}
                      {#if entry.not_before}
                        <div class="ssl-chain-detail">
                          <span class="ssl-chain-detail-label">Valid</span>
                          <span class="ssl-chain-detail-value">{entry.not_before} — {entry.not_after}</span>
                        </div>
                      {/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {:else if certTab === 'raw'}
            <pre class="ssl-raw-output">{certResult.raw_output}</pre>
          {/if}
        </div>
      </div>
    {/if}
  {/if}

  <!-- Decode Certificate Mode -->
  {#if mode === 'decode'}
    <div class="ssl-hint">Paste a PEM certificate to decode its details</div>
    <textarea
      class="ssl-pem-input"
      bind:value={decodePem}
      placeholder="-----BEGIN CERTIFICATE-----&#10;MIICNzCCAZ...&#10;-----END CERTIFICATE-----"
      spellcheck="false"
      rows="6"
    ></textarea>
    <button class="ssl-btn ssl-btn-full" onclick={decodeCert} disabled={decodeLoading || !decodePem.trim()}>
      {#if decodeLoading}
        <svg class="ssl-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        Decoding...
      {:else}
        Decode Certificate
      {/if}
    </button>

    {#if decodeError}
      <div class="ssl-error">{decodeError}</div>
    {/if}

    {#if decodeResult}
      <div class="ssl-results">
        <div class="ssl-tabs">
          <button class="ssl-tab" class:active={decodeTab === 'details'} onclick={() => decodeTab = 'details'}>Details</button>
          <button class="ssl-tab" class:active={decodeTab === 'raw'} onclick={() => decodeTab = 'raw'}>Raw</button>
        </div>
        <div class="ssl-tab-content">
          {#if decodeTab === 'details'}
            <div class="ssl-fields">
              {#if decodeResult.version}
                <div class="ssl-field">
                  <span class="ssl-field-label">Version</span>
                  <span class="ssl-field-value">{decodeResult.version}</span>
                </div>
              {/if}
              <div class="ssl-field">
                <span class="ssl-field-label">Subject</span>
                <span class="ssl-field-value">{decodeResult.subject}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Issuer</span>
                <span class="ssl-field-value">{decodeResult.issuer}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Valid From</span>
                <span class="ssl-field-value">{decodeResult.not_before}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Valid Until</span>
                <span class="ssl-field-value">{decodeResult.not_after}</span>
              </div>
              {#if decodeResult.san.length > 0}
                <div class="ssl-field">
                  <span class="ssl-field-label">SANs</span>
                  <div class="ssl-san-list">
                    {#each decodeResult.san as name}
                      <span class="ssl-san-chip">{name}</span>
                    {/each}
                  </div>
                </div>
              {/if}
              <div class="ssl-field">
                <span class="ssl-field-label">Serial</span>
                <span class="ssl-field-value ssl-mono">{decodeResult.serial}</span>
              </div>
              <div class="ssl-field">
                <span class="ssl-field-label">Fingerprint</span>
                <span class="ssl-field-value ssl-mono ssl-small">{decodeResult.fingerprint}</span>
              </div>
              {#if decodeResult.key_type}
                <div class="ssl-field-row">
                  <div class="ssl-field ssl-field-half">
                    <span class="ssl-field-label">Key Type</span>
                    <span class="ssl-field-value">{decodeResult.key_type}</span>
                  </div>
                  <div class="ssl-field ssl-field-half">
                    <span class="ssl-field-label">Signature</span>
                    <span class="ssl-field-value">{decodeResult.sig_alg}</span>
                  </div>
                </div>
              {/if}
            </div>
          {:else if decodeTab === 'raw'}
            <pre class="ssl-raw-output">{decodeResult.raw_text}</pre>
          {/if}
        </div>
      </div>
    {/if}
  {/if}

  <!-- Hash / Encode Mode -->
  {#if mode === 'hash'}
    <div class="ssl-hint">Hash or encode text using OpenSSL</div>
    <div class="ssl-hash-algo-row">
      {#each HASH_ALGORITHMS as algo}
        <button
          class="ssl-algo-chip"
          class:active={hashAlgorithm === algo.value}
          onclick={() => { hashAlgorithm = algo.value; hashResult = null; }}
        >{algo.label}</button>
      {/each}
    </div>
    <div class="ssl-hash-input-section" onkeydown={handleHashKeydown}>
      <textarea
        class="ssl-hash-input"
        bind:value={hashInput}
        placeholder="Enter text to hash or encode..."
        spellcheck="false"
        rows="4"
      ></textarea>
      <button class="ssl-btn ssl-btn-full" onclick={runHash} disabled={hashLoading || !hashInput.trim()}>
        {#if hashLoading}
          <svg class="ssl-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
        {:else}
          {hashAlgorithm.startsWith('base64') ? 'Encode / Decode' : 'Hash'}
        {/if}
      </button>
    </div>

    {#if hashError}
      <div class="ssl-error">{hashError}</div>
    {/if}

    {#if hashResult}
      <div class="ssl-hash-result">
        <div class="ssl-hash-result-header">
          <span class="ssl-hash-result-algo">{hashResult.algorithm}</span>
          <span class="ssl-hash-result-size">{hashResult.input_size} bytes input</span>
          <button class="ssl-copy-btn" onclick={() => copyToClipboard(hashResult?.output ?? '')}>Copy</button>
        </div>
        <pre class="ssl-hash-output">{hashResult.output}</pre>
      </div>
    {/if}
  {/if}
</div>

<style>
  .ssl-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .ssl-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .ssl-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #50fa7b;
    font-weight: 600;
    font-size: 13px;
  }

  .ssl-logo svg { color: #50fa7b; }

  .ssl-mode-toggle {
    display: flex;
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    overflow: hidden;
  }

  .ssl-mode-btn {
    padding: 4px 10px;
    font-size: 10px;
    font-family: inherit;
    color: #6e6e82;
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 0.15s;
  }

  .ssl-mode-btn.active {
    background: #1a2e1a;
    color: #50fa7b;
  }

  .ssl-mode-btn:hover:not(.active) { color: #b4b4c4; }

  .ssl-hint {
    font-size: 10px;
    color: #6e6e82;
    line-height: 1.4;
  }

  .ssl-input-row {
    display: flex;
    gap: 6px;
  }

  .ssl-input {
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

  .ssl-input:focus { border-color: #3a3a5a; }
  .ssl-input::placeholder { color: #4a4a5e; }

  .ssl-host-input { flex: 1; min-width: 0; }
  .ssl-port-input { width: 60px; text-align: center; }

  .ssl-btn {
    background: #1a2e1a;
    border: 1px solid #2a3e2a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #50fa7b;
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

  .ssl-btn:hover:not(:disabled) { background: #1e3a1e; }
  .ssl-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .ssl-btn-full { width: 100%; justify-content: center; }

  .ssl-spinner { animation: ssl-spin 1s linear infinite; }
  @keyframes ssl-spin { to { transform: rotate(360deg); } }

  .ssl-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  /* ── Expiry banner ── */
  .ssl-expiry-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid;
    border-radius: 6px;
  }

  .ssl-expiry-icon { flex-shrink: 0; }

  .ssl-expiry-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ssl-expiry-label {
    font-size: 12px;
    font-weight: 600;
  }

  .ssl-expiry-date {
    font-size: 10px;
    color: #6e6e82;
  }

  /* ── Tabs ── */
  .ssl-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .ssl-tabs {
    display: flex;
    gap: 0;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px 6px 0 0;
    padding: 0 4px;
    border-bottom: 1px solid #1e1e2e;
  }

  .ssl-tab {
    padding: 6px 10px;
    font-size: 11px;
    font-family: inherit;
    color: #6e6e82;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .ssl-tab.active {
    color: #e4e4ed;
    border-bottom-color: #50fa7b;
  }

  .ssl-tab:hover:not(.active) { color: #b4b4c4; }

  .ssl-tab-badge {
    font-size: 9px;
    background: #2a2a3a;
    color: #8888a0;
    padding: 1px 5px;
    border-radius: 8px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .ssl-tab-content {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-top: none;
    border-radius: 0 0 6px 6px;
    overflow: hidden;
  }

  /* ── Fields ── */
  .ssl-fields {
    display: flex;
    flex-direction: column;
  }

  .ssl-field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 10px;
    border-bottom: 1px solid #14141e;
  }

  .ssl-field:last-child { border-bottom: none; }

  .ssl-field-row {
    display: flex;
    gap: 0;
    border-bottom: 1px solid #14141e;
  }

  .ssl-field-row:last-child { border-bottom: none; }

  .ssl-field-half {
    flex: 1;
    border-bottom: none;
  }

  .ssl-field-half + .ssl-field-half {
    border-left: 1px solid #14141e;
  }

  .ssl-field-label {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
    font-weight: 600;
  }

  .ssl-field-value {
    font-size: 11px;
    color: #e4e4ed;
    word-break: break-all;
  }

  .ssl-mono {
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .ssl-small { font-size: 10px; }

  .ssl-san-list {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .ssl-san-chip {
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    padding: 2px 6px;
    background: #8be9fd12;
    color: #8be9fd;
    border: 1px solid #8be9fd25;
    border-radius: 4px;
  }

  /* ── Chain ── */
  .ssl-chain {
    display: flex;
    flex-direction: column;
    padding: 8px;
  }

  .ssl-chain-entry {
    display: flex;
    gap: 10px;
  }

  .ssl-chain-connector {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 14px;
    flex-shrink: 0;
    padding-top: 10px;
  }

  .ssl-chain-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    z-index: 1;
  }

  .ssl-chain-line {
    width: 2px;
    flex: 1;
    background: #2a2a3a;
    margin-top: 2px;
  }

  .ssl-chain-card {
    flex: 1;
    min-width: 0;
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    margin-bottom: 6px;
    overflow: hidden;
  }

  .ssl-chain-leaf .ssl-chain-card {
    border-color: #8be9fd25;
  }

  .ssl-chain-card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid #14141e;
  }

  .ssl-chain-depth {
    font-size: 9px;
    font-weight: 700;
    color: #6e6e82;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .ssl-chain-subject {
    font-size: 11px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .ssl-chain-card-body {
    display: flex;
    flex-direction: column;
  }

  .ssl-chain-detail {
    display: flex;
    gap: 8px;
    padding: 3px 10px;
    font-size: 10px;
    border-bottom: 1px solid #0e0e18;
  }

  .ssl-chain-detail:last-child { border-bottom: none; }

  .ssl-chain-detail-label {
    color: #6e6e82;
    min-width: 55px;
    flex-shrink: 0;
  }

  .ssl-chain-detail-value {
    color: #8888a0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* ── Raw output ── */
  .ssl-raw-output {
    padding: 10px;
    margin: 0;
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #8888a0;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 400px;
    overflow-y: auto;
    line-height: 1.5;
  }

  /* ── PEM input ── */
  .ssl-pem-input {
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 8px 10px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
    resize: vertical;
    min-height: 80px;
  }

  .ssl-pem-input:focus { border-color: #3a3a5a; }
  .ssl-pem-input::placeholder { color: #3a3a4e; }

  /* ── Hash mode ── */
  .ssl-hash-algo-row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .ssl-algo-chip {
    padding: 4px 10px;
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #6e6e82;
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .ssl-algo-chip:hover { color: #b4b4c4; border-color: #2a2a3a; }

  .ssl-algo-chip.active {
    background: #1a2e1a;
    color: #50fa7b;
    border-color: #2a3e2a;
  }

  .ssl-hash-input-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .ssl-hash-input {
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 8px 10px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
    resize: vertical;
    min-height: 60px;
  }

  .ssl-hash-input:focus { border-color: #3a3a5a; }
  .ssl-hash-input::placeholder { color: #4a4a5e; }

  .ssl-hash-result {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    overflow: hidden;
  }

  .ssl-hash-result-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid #1a1a28;
  }

  .ssl-hash-result-algo {
    font-size: 10px;
    font-weight: 700;
    color: #50fa7b;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    text-transform: uppercase;
  }

  .ssl-hash-result-size {
    font-size: 9px;
    color: #6e6e82;
    flex: 1;
  }

  .ssl-copy-btn {
    font-size: 9px;
    color: #6e6e82;
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    padding: 2px 8px;
    cursor: pointer;
    font-family: inherit;
    transition: all 0.15s;
  }

  .ssl-copy-btn:hover {
    color: #8be9fd;
    border-color: #2a3a4a;
  }

  .ssl-hash-output {
    padding: 10px;
    margin: 0;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #e4e4ed;
    white-space: pre-wrap;
    word-break: break-all;
    line-height: 1.5;
    user-select: all;
  }
</style>
