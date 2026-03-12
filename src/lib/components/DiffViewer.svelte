<script lang="ts">
  import { highlight } from '$lib/highlight';

  interface DiffLine {
    tag: string;
    content: string;
    old_line: number | null;
    new_line: number | null;
  }

  interface Props {
    fileName: string;
    diffLines: DiffLine[];
    summary: string;
    onApply: () => void;
    onReject: () => void;
    applying?: boolean;
  }

  let { fileName, diffLines, summary, onApply, onReject, applying = false }: Props = $props();

  // Get file extension for syntax highlighting
  const langMap: Record<string, string> = {
    ts: 'typescript',
    tsx: 'tsx',
    js: 'javascript',
    jsx: 'jsx',
    svelte: 'svelte',
    rs: 'rust',
    py: 'python',
    go: 'go',
    css: 'css',
    html: 'html',
    json: 'json',
    md: 'markdown',
    yaml: 'yaml',
    yml: 'yaml',
    toml: 'toml',
    sh: 'bash',
    bash: 'bash',
  };
  
  let lang = $derived(langMap[fileName.split('.').pop() || 'txt'] || 'plaintext');

  function highlightLine(content: string): string {
    try {
      // Remove trailing newline for display
      const trimmed = content.replace(/\n$/, '');
      return highlight(trimmed, lang);
    } catch {
      return content.replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }
  }
</script>

<div class="diff-viewer">
  <div class="diff-header">
    <div class="diff-title">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
      </svg>
      <span>{fileName}</span>
    </div>
    <div class="diff-actions">
      <button class="diff-btn reject" onclick={onReject} disabled={applying}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
        Reject
      </button>
      <button class="diff-btn apply" onclick={onApply} disabled={applying}>
        {#if applying}
          <svg class="spinner" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" stroke-dasharray="32" stroke-dashoffset="12"/>
          </svg>
          Applying...
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
          Apply Changes
        {/if}
      </button>
    </div>
  </div>

  <div class="diff-summary">
    {summary}
  </div>

  <div class="diff-content">
    {#each diffLines as line}
      <div class="diff-line {line.tag}">
        <span class="line-num old">{line.old_line ?? ''}</span>
        <span class="line-num new">{line.new_line ?? ''}</span>
        <span class="line-marker">
          {#if line.tag === 'insert'}+{:else if line.tag === 'delete'}-{:else}&nbsp;{/if}
        </span>
        <span class="line-content">{@html highlightLine(line.content)}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .diff-viewer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-primary, #1e1e1e);
    color: var(--text-primary, #e0e0e0);
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 13px;
  }

  .diff-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: var(--bg-secondary, #252525);
    border-bottom: 1px solid var(--border-color, #333);
  }

  .diff-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
  }

  .diff-title svg {
    opacity: 0.7;
  }

  .diff-actions {
    display: flex;
    gap: 8px;
  }

  .diff-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .diff-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .diff-btn.reject {
    background: rgba(255, 100, 100, 0.15);
    color: #ff6b6b;
  }

  .diff-btn.reject:hover:not(:disabled) {
    background: rgba(255, 100, 100, 0.25);
  }

  .diff-btn.apply {
    background: rgba(100, 255, 150, 0.15);
    color: #6bffb8;
  }

  .diff-btn.apply:hover:not(:disabled) {
    background: rgba(100, 255, 150, 0.25);
  }

  .diff-summary {
    padding: 10px 16px;
    background: var(--bg-tertiary, #2a2a2a);
    border-bottom: 1px solid var(--border-color, #333);
    font-size: 12px;
    color: var(--text-secondary, #aaa);
    white-space: pre-wrap;
  }

  .diff-content {
    flex: 1;
    overflow: auto;
    padding: 8px 0;
  }

  .diff-line {
    display: flex;
    line-height: 1.5;
    min-height: 20px;
  }

  .diff-line.insert {
    background: rgba(80, 200, 120, 0.15);
  }

  .diff-line.delete {
    background: rgba(255, 100, 100, 0.15);
  }

  .diff-line.equal {
    background: transparent;
  }

  .line-num {
    width: 40px;
    padding: 0 8px;
    text-align: right;
    color: var(--text-tertiary, #666);
    user-select: none;
    flex-shrink: 0;
  }

  .line-num.old {
    background: rgba(0, 0, 0, 0.1);
  }

  .line-num.new {
    background: rgba(0, 0, 0, 0.05);
  }

  .line-marker {
    width: 20px;
    text-align: center;
    flex-shrink: 0;
    font-weight: bold;
  }

  .diff-line.insert .line-marker {
    color: #6bffb8;
  }

  .diff-line.delete .line-marker {
    color: #ff6b6b;
  }

  .line-content {
    flex: 1;
    padding-right: 16px;
    white-space: pre;
    overflow-x: auto;
  }

  .spinner {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
