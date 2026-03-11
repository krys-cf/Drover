<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface CurlHeader {
    name: string;
    value: string;
  }

  interface CurlTimings {
    dns_lookup: string;
    tcp_connect: string;
    tls_handshake: string;
    ttfb: string;
    total: string;
  }

  interface CurlRedirect {
    status: number;
    location: string;
  }

  interface CurlResult {
    url: string;
    method: string;
    status_code: number;
    status_text: string;
    response_headers: CurlHeader[];
    body: string;
    body_size: number;
    timings: CurlTimings;
    redirects: CurlRedirect[];
    remote_ip: string;
    content_type: string;
    verbose_output: string;
  }

  interface RequestHeader {
    key: string;
    value: string;
    enabled: boolean;
  }

  let url = $state('');
  let method = $state('GET');
  let requestBody = $state('');
  let followRedirects = $state(true);
  let requestHeaders = $state<RequestHeader[]>([{ key: '', value: '', enabled: true }]);
  let authType = $state<'none' | 'bearer' | 'basic'>('none');
  let authToken = $state('');
  let authUser = $state('');
  let authPass = $state('');
  let rawFlags = $state('');
  let userAgent = $state('');
  let insecure = $state(false);
  let headersOnly = $state(false);
  let verbose = $state(false);

  let result: CurlResult | null = $state(null);
  let loading = $state(false);
  let error = $state('');

  let requestTab = $state<'headers' | 'body' | 'auth' | 'options'>('headers');
  let responseTab = $state<'body' | 'headers' | 'timings' | 'verbose'>('body');

  interface FlagPreset {
    label: string;
    flags: string;
    description: string;
  }

  const FLAG_PRESETS: FlagPreset[] = [
    { label: '-v', flags: '-v', description: 'Verbose output' },
    { label: '-k', flags: '-k', description: 'Allow insecure SSL' },
    { label: '-I', flags: '-I', description: 'Headers only' },
    { label: '-o /dev/null', flags: '-o /dev/null', description: 'Discard body' },
    { label: '-svo /dev/null', flags: '-svo /dev/null', description: 'Silent verbose, discard body' },
    { label: '--compressed', flags: '--compressed', description: 'Request compressed response' },
  ];

  const UA_PRESETS: Record<string, string> = {
    Chrome: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36',
    Firefox: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:122.0) Gecko/20100101 Firefox/122.0',
    Safari: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15',
    curl: 'curl/8.4.0',
    Bot: 'Googlebot/2.1 (+http://www.google.com/bot.html)',
  };

  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

  const METHOD_COLORS: Record<string, string> = {
    GET: '#50fa7b',
    POST: '#ffb86c',
    PUT: '#8be9fd',
    PATCH: '#bd93f9',
    DELETE: '#ff5555',
    HEAD: '#6272a4',
    OPTIONS: '#f1fa8c',
  };

  function statusColor(code: number): string {
    if (code >= 200 && code < 300) return '#50fa7b';
    if (code >= 300 && code < 400) return '#8be9fd';
    if (code >= 400 && code < 500) return '#ffb86c';
    if (code >= 500) return '#ff5555';
    return '#6e6e82';
  }

  function formatBodySize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  }

  function isJsonContent(ct: string): boolean {
    return ct.includes('json');
  }

  function isHtmlContent(ct: string): boolean {
    return ct.includes('html');
  }

  function isXmlContent(ct: string): boolean {
    return ct.includes('xml');
  }

  function prettyPrintBody(body: string, contentType: string): string {
    if (isJsonContent(contentType)) {
      try {
        return JSON.stringify(JSON.parse(body), null, 2);
      } catch {
        return body;
      }
    }
    return body;
  }

  function bodyLanguage(contentType: string): string {
    if (isJsonContent(contentType)) return 'json';
    if (isHtmlContent(contentType)) return 'html';
    if (isXmlContent(contentType)) return 'xml';
    return 'text';
  }

  function addHeader() {
    requestHeaders.push({ key: '', value: '', enabled: true });
  }

  function removeHeader(index: number) {
    requestHeaders.splice(index, 1);
    if (requestHeaders.length === 0) {
      requestHeaders.push({ key: '', value: '', enabled: true });
    }
  }

  function buildHeaders(): string[] {
    const hdrs: string[] = [];

    // Add auth header
    if (authType === 'bearer' && authToken.trim()) {
      hdrs.push(`Authorization: Bearer ${authToken.trim()}`);
    } else if (authType === 'basic' && authUser.trim()) {
      const encoded = btoa(`${authUser}:${authPass}`);
      hdrs.push(`Authorization: Basic ${encoded}`);
    }

    // Add user-agent
    if (userAgent.trim()) {
      hdrs.push(`User-Agent: ${userAgent.trim()}`);
    }

    // Add custom headers
    for (const h of requestHeaders) {
      if (h.enabled && h.key.trim()) {
        hdrs.push(`${h.key.trim()}: ${h.value}`);
      }
    }

    return hdrs;
  }

  function buildExtraFlags(): string[] {
    const flags: string[] = [];
    if (insecure) flags.push('-k');
    if (headersOnly) flags.push('-I');
    if (verbose) flags.push('-v');
    if (rawFlags.trim()) flags.push(rawFlags.trim());
    return flags;
  }

  function togglePresetFlag(preset: FlagPreset) {
    const current = rawFlags.trim();
    if (current.includes(preset.flags)) {
      rawFlags = current.replace(preset.flags, '').replace(/\s+/g, ' ').trim();
    } else {
      rawFlags = current ? `${current} ${preset.flags}` : preset.flags;
    }
  }

  function hasPresetFlag(preset: FlagPreset): boolean {
    return rawFlags.includes(preset.flags);
  }

  async function runCurl() {
    if (!url.trim()) return;

    let finalUrl = url.trim();
    if (!finalUrl.startsWith('http://') && !finalUrl.startsWith('https://')) {
      finalUrl = 'https://' + finalUrl;
    }

    loading = true;
    error = '';
    result = null;

    try {
      const extraFlags = buildExtraFlags();
      result = await invoke<CurlResult>('run_curl', {
        url: finalUrl,
        method,
        headers: buildHeaders(),
        body: methodHasBody(method) ? requestBody : null,
        followRedirects,
        extraFlags: extraFlags.length > 0 ? extraFlags : null,
      });
      responseTab = 'body';
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function methodHasBody(m: string): boolean {
    return m === 'POST' || m === 'PUT' || m === 'PATCH';
  }

  function timingPercent(value: string, total: string): number {
    const v = parseFloat(value);
    const t = parseFloat(total);
    if (isNaN(v) || isNaN(t) || t === 0) return 0;
    return Math.min(100, Math.round((v / t) * 100));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      runCurl();
    }
  }
</script>

<div class="curl-app" onkeydown={handleKeydown}>
  <div class="curl-header">
    <div class="curl-logo">
      <svg viewBox="0 0 24 24" width="15" height="15" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path><polyline points="15 3 21 3 21 9"></polyline><line x1="10" y1="14" x2="21" y2="3"></line></svg>
      <span>Curl</span>
    </div>
  </div>

  <!-- URL bar -->
  <div class="curl-url-row">
    <select class="curl-method" bind:value={method} style="color: {METHOD_COLORS[method] || '#e4e4ed'}">
      {#each METHODS as m}
        <option value={m} style="color: {METHOD_COLORS[m]}">{m}</option>
      {/each}
    </select>
    <input
      class="curl-url"
      type="text"
      bind:value={url}
      placeholder="https://api.example.com/endpoint"
      spellcheck="false"
      autocomplete="off"
    />
    <button class="curl-send" onclick={runCurl} disabled={loading || !url.trim()}>
      {#if loading}
        <svg class="curl-spinner" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
      {:else}
        Send
      {/if}
    </button>
  </div>

  <!-- Request config tabs -->
  <div class="curl-section">
    <div class="curl-tabs">
      <button class="curl-tab" class:active={requestTab === 'headers'} onclick={() => requestTab = 'headers'}>
        Headers
        {#if requestHeaders.some(h => h.key.trim())}
          <span class="curl-tab-badge">{requestHeaders.filter(h => h.enabled && h.key.trim()).length}</span>
        {/if}
      </button>
      <button class="curl-tab" class:active={requestTab === 'body'} onclick={() => requestTab = 'body'}>Body</button>
      <button class="curl-tab" class:active={requestTab === 'auth'} onclick={() => requestTab = 'auth'}>
        Auth
        {#if authType !== 'none'}
          <span class="curl-tab-dot"></span>
        {/if}
      </button>
      <button class="curl-tab" class:active={requestTab === 'options'} onclick={() => requestTab = 'options'}>
        Options
        {#if rawFlags.trim() || insecure || headersOnly || verbose || userAgent.trim()}
          <span class="curl-tab-dot"></span>
        {/if}
      </button>
    </div>

    <div class="curl-tab-content">
      {#if requestTab === 'headers'}
        <div class="curl-headers-list">
          {#each requestHeaders as header, i}
            <div class="curl-header-row">
              <input type="checkbox" bind:checked={header.enabled} class="curl-header-check" />
              <input class="curl-header-key" type="text" bind:value={header.key} placeholder="Header name" spellcheck="false" />
              <input class="curl-header-val" type="text" bind:value={header.value} placeholder="Value" spellcheck="false" />
              <button class="curl-header-remove" onclick={() => removeHeader(i)} title="Remove">&times;</button>
            </div>
          {/each}
          <button class="curl-add-header" onclick={addHeader}>+ Add header</button>
        </div>
      {:else if requestTab === 'body'}
        <div class="curl-body-section">
          {#if !methodHasBody(method)}
            <div class="curl-body-hint">Body is only sent with POST, PUT, and PATCH requests</div>
          {/if}
          <textarea
            class="curl-body-input"
            bind:value={requestBody}
            placeholder="key: value"
            spellcheck="false"
            rows="6"
          ></textarea>
        </div>
      {:else if requestTab === 'auth'}
        <div class="curl-auth-section">
          <div class="curl-auth-type-row">
            <button class="curl-auth-type" class:active={authType === 'none'} onclick={() => authType = 'none'}>None</button>
            <button class="curl-auth-type" class:active={authType === 'bearer'} onclick={() => authType = 'bearer'}>Bearer Token</button>
            <button class="curl-auth-type" class:active={authType === 'basic'} onclick={() => authType = 'basic'}>Basic Auth</button>
          </div>
          {#if authType === 'bearer'}
            <input class="curl-auth-input" type="text" bind:value={authToken} placeholder="Token" spellcheck="false" />
          {:else if authType === 'basic'}
            <div class="curl-auth-basic">
              <input class="curl-auth-input" type="text" bind:value={authUser} placeholder="Username" spellcheck="false" />
              <input class="curl-auth-input" type="password" bind:value={authPass} placeholder="Password" spellcheck="false" />
            </div>
          {:else}
            <div class="curl-auth-none">No authentication</div>
          {/if}
        </div>
      {:else if requestTab === 'options'}
        <div class="curl-options-section">
          <div class="curl-options-toggles">
            <label class="curl-toggle-label">
              <input type="checkbox" bind:checked={followRedirects} />
              <span>Follow redirects (-L)</span>
            </label>
            <label class="curl-toggle-label">
              <input type="checkbox" bind:checked={insecure} />
              <span>Insecure SSL (-k)</span>
            </label>
            <label class="curl-toggle-label">
              <input type="checkbox" bind:checked={headersOnly} />
              <span>Headers only (-I)</span>
            </label>
            <label class="curl-toggle-label">
              <input type="checkbox" bind:checked={verbose} />
              <span>Verbose (-v)</span>
            </label>
          </div>

          <div class="curl-options-group">
            <div class="curl-options-label">Common Flags</div>
            <div class="curl-flag-presets">
              {#each FLAG_PRESETS as preset}
                <button
                  class="curl-flag-chip"
                  class:active={hasPresetFlag(preset)}
                  onclick={() => togglePresetFlag(preset)}
                  title={preset.description}
                >{preset.label}</button>
              {/each}
            </div>
          </div>

          <div class="curl-options-group">
            <div class="curl-options-label">User-Agent</div>
            <div class="curl-ua-presets">
              {#each Object.entries(UA_PRESETS) as [name, ua]}
                <button
                  class="curl-ua-chip"
                  class:active={userAgent === ua}
                  onclick={() => userAgent = userAgent === ua ? '' : ua}
                >{name}</button>
              {/each}
            </div>
            <input class="curl-options-input" type="text" bind:value={userAgent} placeholder="Custom user-agent string" spellcheck="false" />
          </div>

          <div class="curl-options-group">
            <div class="curl-options-label">Raw Flags</div>
            <input class="curl-options-input" type="text" bind:value={rawFlags} placeholder="e.g. -svo /dev/null --connect-timeout 5" spellcheck="false" />
            <div class="curl-options-hint">Additional curl flags passed directly to the command</div>
          </div>
        </div>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="curl-error">{error}</div>
  {/if}

  <!-- Response -->
  {#if result}
    <div class="curl-response">
      <!-- Status bar -->
      <div class="curl-status-bar">
        <span class="curl-status-badge" style="background: {statusColor(result.status_code)}20; color: {statusColor(result.status_code)}; border-color: {statusColor(result.status_code)}40">
          {result.status_code} {result.status_text}
        </span>
        <span class="curl-status-meta">{result.timings.total}</span>
        <span class="curl-status-meta">{formatBodySize(result.body_size)}</span>
        {#if result.remote_ip}
          <span class="curl-status-meta">{result.remote_ip}</span>
        {/if}
        {#if result.redirects.length > 0}
          <span class="curl-status-meta curl-redirect-count">{result.redirects.length} redirect{result.redirects.length > 1 ? 's' : ''}</span>
        {/if}
      </div>

      <!-- Redirect chain -->
      {#if result.redirects.length > 0}
        <div class="curl-redirects">
          {#each result.redirects as redir, i}
            <div class="curl-redirect-step">
              <span class="curl-redirect-badge" style="color: {statusColor(redir.status)}">{redir.status}</span>
              <svg viewBox="0 0 24 24" width="10" height="10" stroke="currentColor" stroke-width="1.5" fill="none"><polyline points="9 18 15 12 9 6"></polyline></svg>
              <span class="curl-redirect-url">{redir.location}</span>
            </div>
          {/each}
          <div class="curl-redirect-step curl-redirect-final">
            <span class="curl-redirect-badge" style="color: {statusColor(result.status_code)}">{result.status_code}</span>
            <span class="curl-redirect-url">{result.url}</span>
          </div>
        </div>
      {/if}

      <!-- Response tabs -->
      <div class="curl-tabs">
        <button class="curl-tab" class:active={responseTab === 'body'} onclick={() => responseTab = 'body'}>
          Body
          {#if result.content_type}
            <span class="curl-tab-lang">{bodyLanguage(result.content_type)}</span>
          {/if}
        </button>
        <button class="curl-tab" class:active={responseTab === 'headers'} onclick={() => responseTab = 'headers'}>
          Headers
          <span class="curl-tab-badge">{result.response_headers.length}</span>
        </button>
        <button class="curl-tab" class:active={responseTab === 'timings'} onclick={() => responseTab = 'timings'}>Timings</button>
        {#if result.verbose_output}
          <button class="curl-tab" class:active={responseTab === 'verbose'} onclick={() => responseTab = 'verbose'}>Verbose</button>
        {/if}
      </div>

      <div class="curl-response-content">
        {#if responseTab === 'body'}
          <pre class="curl-body-output">{prettyPrintBody(result.body, result.content_type)}</pre>
        {:else if responseTab === 'headers'}
          <div class="curl-response-headers">
            {#each result.response_headers as header}
              <div class="curl-resp-header">
                <span class="curl-resp-header-name">{header.name}</span>
                <span class="curl-resp-header-value">{header.value}</span>
              </div>
            {/each}
          </div>
        {:else if responseTab === 'timings'}
          <div class="curl-timings">
            <div class="curl-timing-row">
              <span class="curl-timing-label">DNS Lookup</span>
              <div class="curl-timing-bar-wrap">
                <div class="curl-timing-bar" style="background: #bd93f9; width: {timingPercent(result.timings.dns_lookup, result.timings.total)}%"></div>
              </div>
              <span class="curl-timing-value">{result.timings.dns_lookup}</span>
            </div>
            <div class="curl-timing-row">
              <span class="curl-timing-label">TCP Connect</span>
              <div class="curl-timing-bar-wrap">
                <div class="curl-timing-bar" style="background: #8be9fd; width: {timingPercent(result.timings.tcp_connect, result.timings.total)}%"></div>
              </div>
              <span class="curl-timing-value">{result.timings.tcp_connect}</span>
            </div>
            <div class="curl-timing-row">
              <span class="curl-timing-label">TLS Handshake</span>
              <div class="curl-timing-bar-wrap">
                <div class="curl-timing-bar" style="background: #50fa7b; width: {timingPercent(result.timings.tls_handshake, result.timings.total)}%"></div>
              </div>
              <span class="curl-timing-value">{result.timings.tls_handshake}</span>
            </div>
            <div class="curl-timing-row">
              <span class="curl-timing-label">Time to First Byte</span>
              <div class="curl-timing-bar-wrap">
                <div class="curl-timing-bar" style="background: #ffb86c; width: {timingPercent(result.timings.ttfb, result.timings.total)}%"></div>
              </div>
              <span class="curl-timing-value">{result.timings.ttfb}</span>
            </div>
            <div class="curl-timing-row curl-timing-total">
              <span class="curl-timing-label">Total</span>
              <div class="curl-timing-bar-wrap">
                <div class="curl-timing-bar" style="background: #e4e4ed; width: 100%"></div>
              </div>
              <span class="curl-timing-value">{result.timings.total}</span>
            </div>
          </div>
        {:else if responseTab === 'verbose'}
          <pre class="curl-verbose-output">{result.verbose_output}</pre>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .curl-app {
    padding: 12px;
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .curl-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .curl-logo {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #ffb86c;
    font-weight: 600;
    font-size: 13px;
  }

  .curl-logo svg { color: #ffb86c; }

  /* ── URL bar ── */
  .curl-url-row {
    display: flex;
    gap: 6px;
  }

  .curl-method {
    background: #111118;
    border: 1px solid #2a2a3a;
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 11px;
    font-weight: 700;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
    cursor: pointer;
    min-width: 72px;
  }

  .curl-method:focus { border-color: #3a3a5a; }

  .curl-url {
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

  .curl-url:focus { border-color: #3a3a5a; }
  .curl-url::placeholder { color: #4a4a5e; }

  .curl-send {
    background: #1e2a3a;
    border: 1px solid #2a3a4a;
    border-radius: 6px;
    padding: 6px 14px;
    color: #8be9fd;
    font-size: 11px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    transition: background 0.15s, border-color 0.15s;
  }

  .curl-send:hover:not(:disabled) {
    background: #253545;
    border-color: #3a4a5a;
  }

  .curl-send:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .curl-spinner {
    animation: curl-spin 1s linear infinite;
  }

  @keyframes curl-spin {
    to { transform: rotate(360deg); }
  }

  /* ── Tabs ── */
  .curl-section {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    overflow: hidden;
  }

  .curl-tabs {
    display: flex;
    align-items: center;
    gap: 0;
    border-bottom: 1px solid #1e1e2e;
    padding: 0 4px;
  }

  .curl-tab {
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

  .curl-tab.active {
    color: #e4e4ed;
    border-bottom-color: #8be9fd;
  }

  .curl-tab:hover:not(.active) { color: #b4b4c4; }

  .curl-tab-badge {
    font-size: 9px;
    background: #2a2a3a;
    color: #8888a0;
    padding: 1px 5px;
    border-radius: 8px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .curl-tab-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #50fa7b;
  }

  .curl-tab-lang {
    font-size: 9px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .curl-toggle-label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    color: #6e6e82;
    cursor: pointer;
    padding: 4px 6px;
    user-select: none;
  }

  .curl-toggle-label input[type="checkbox"] {
    width: 12px;
    height: 12px;
    accent-color: #8be9fd;
  }

  .curl-tab-content {
    padding: 8px;
  }

  /* ── Headers editor ── */
  .curl-headers-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .curl-header-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .curl-header-check {
    width: 12px;
    height: 12px;
    accent-color: #8be9fd;
    flex-shrink: 0;
  }

  .curl-header-key,
  .curl-header-val {
    flex: 1;
    min-width: 0;
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    padding: 4px 8px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
  }

  .curl-header-key:focus,
  .curl-header-val:focus { border-color: #3a3a5a; }

  .curl-header-key::placeholder,
  .curl-header-val::placeholder { color: #3a3a4e; }

  .curl-header-remove {
    background: none;
    border: none;
    color: #6e6e82;
    font-size: 14px;
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
  }

  .curl-header-remove:hover { color: #ff5555; }

  .curl-add-header {
    background: none;
    border: none;
    color: #6e6e82;
    font-size: 10px;
    cursor: pointer;
    padding: 4px 0;
    text-align: left;
    font-family: inherit;
  }

  .curl-add-header:hover { color: #8be9fd; }

  /* ── Body editor ── */
  .curl-body-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .curl-body-hint {
    font-size: 10px;
    color: #6e6e82;
    font-style: italic;
  }

  .curl-body-input {
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    padding: 8px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
    resize: vertical;
    min-height: 60px;
  }

  .curl-body-input:focus { border-color: #3a3a5a; }
  .curl-body-input::placeholder { color: #3a3a4e; }

  /* ── Auth ── */
  .curl-auth-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .curl-auth-type-row {
    display: flex;
    gap: 4px;
  }

  .curl-auth-type {
    padding: 4px 10px;
    font-size: 10px;
    font-family: inherit;
    color: #6e6e82;
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .curl-auth-type.active {
    background: #1e2a3a;
    color: #8be9fd;
    border-color: #2a3a4a;
  }

  .curl-auth-input {
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    padding: 6px 8px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
  }

  .curl-auth-input:focus { border-color: #3a3a5a; }
  .curl-auth-input::placeholder { color: #3a3a4e; }

  .curl-auth-basic {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .curl-auth-none {
    font-size: 10px;
    color: #6e6e82;
    padding: 4px 0;
  }

  /* ── Error ── */
  .curl-error {
    background: #ff555520;
    border: 1px solid #ff555540;
    border-radius: 6px;
    padding: 8px 10px;
    color: #ff5555;
    font-size: 11px;
  }

  /* ── Response ── */
  .curl-response {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .curl-status-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .curl-status-badge {
    font-size: 11px;
    font-weight: 700;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    padding: 3px 10px;
    border-radius: 4px;
    border: 1px solid;
  }

  .curl-status-meta {
    font-size: 10px;
    color: #6e6e82;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .curl-redirect-count {
    color: #8be9fd;
  }

  /* ── Redirects ── */
  .curl-redirects {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .curl-redirect-step {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #8888a0;
  }

  .curl-redirect-step svg { color: #6e6e82; flex-shrink: 0; }

  .curl-redirect-badge {
    font-weight: 700;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 24px;
  }

  .curl-redirect-url {
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .curl-redirect-final {
    color: #e4e4ed;
  }

  /* ── Response content ── */
  .curl-response .curl-tabs {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-radius: 6px 6px 0 0;
    border-bottom: 1px solid #1e1e2e;
  }

  .curl-response-content {
    background: #111118;
    border: 1px solid #1e1e2e;
    border-top: none;
    border-radius: 0 0 6px 6px;
    overflow: hidden;
  }

  .curl-body-output {
    padding: 10px;
    margin: 0;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #e4e4ed;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 400px;
    overflow-y: auto;
    line-height: 1.5;
  }

  /* ── Response headers ── */
  .curl-response-headers {
    display: flex;
    flex-direction: column;
  }

  .curl-resp-header {
    display: flex;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid #14141e;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
  }

  .curl-resp-header:last-child { border-bottom: none; }

  .curl-resp-header-name {
    color: #8be9fd;
    min-width: 120px;
    flex-shrink: 0;
    font-weight: 600;
  }

  .curl-resp-header-value {
    color: #e4e4ed;
    word-break: break-all;
    min-width: 0;
  }

  /* ── Timings ── */
  .curl-timings {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }

  .curl-timing-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .curl-timing-label {
    font-size: 10px;
    color: #8888a0;
    min-width: 110px;
    flex-shrink: 0;
  }

  .curl-timing-bar-wrap {
    flex: 1;
    height: 6px;
    background: #1a1a28;
    border-radius: 3px;
    overflow: hidden;
  }

  .curl-timing-bar {
    height: 100%;
    border-radius: 3px;
    min-width: 2px;
    transition: width 0.3s ease;
  }

  .curl-timing-value {
    font-size: 10px;
    color: #e4e4ed;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    min-width: 50px;
    text-align: right;
    flex-shrink: 0;
  }

  .curl-timing-total .curl-timing-label {
    color: #e4e4ed;
    font-weight: 600;
  }

  /* ── Options section ── */
  .curl-options-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .curl-options-toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
  }

  .curl-options-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .curl-options-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6e6e82;
    font-weight: 600;
  }

  .curl-flag-presets,
  .curl-ua-presets {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .curl-flag-chip,
  .curl-ua-chip {
    padding: 3px 8px;
    font-size: 10px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    color: #6e6e82;
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .curl-flag-chip:hover,
  .curl-ua-chip:hover {
    color: #b4b4c4;
    border-color: #2a2a3a;
  }

  .curl-flag-chip.active,
  .curl-ua-chip.active {
    background: #1e2a3a;
    color: #8be9fd;
    border-color: #2a3a4a;
  }

  .curl-options-input {
    background: #0a0a12;
    border: 1px solid #1e1e2e;
    border-radius: 4px;
    padding: 5px 8px;
    color: #e4e4ed;
    font-size: 11px;
    font-family: 'JetBrains Mono', 'Fira Code', 'SF Mono', Menlo, monospace;
    outline: none;
  }

  .curl-options-input:focus { border-color: #3a3a5a; }
  .curl-options-input::placeholder { color: #3a3a4e; }

  .curl-options-hint {
    font-size: 9px;
    color: #4a4a5e;
  }

  /* ── Verbose output ── */
  .curl-verbose-output {
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
</style>
