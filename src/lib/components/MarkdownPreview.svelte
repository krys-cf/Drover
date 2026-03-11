<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { renderMarkdown, stripFrontmatter, preprocessMdx } from '$lib/markdown';

  let {
    filePath = $bindable(''),
    fileName = $bindable(''),
    loading = $bindable(false),
    error = $bindable(''),
    sshTarget = '',
    onClose,
    onEdit,
  }: {
    filePath: string;
    fileName: string;
    loading: boolean;
    error: string;
    sshTarget: string;
    onClose: () => void;
    onEdit: () => void;
  } = $props();

  let rawContent = $state('');
  let renderedHtml = $state('');
  let frontmatter = $state<Record<string, string> | null>(null);

  export async function openFile(path: string, name: string) {
    filePath = path;
    fileName = name;
    rawContent = '';
    renderedHtml = '';
    frontmatter = null;
    error = '';
    loading = true;
    try {
      const fileContent: string = sshTarget
        ? await invoke('read_remote_file_contents', { sshTarget, path })
        : await invoke('read_file_contents', { path });
      rawContent = fileContent;
      const { body, meta } = stripFrontmatter(fileContent);
      frontmatter = meta;
      const processed = preprocessMdx(body);
      renderedHtml = renderMarkdown(processed);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="markdown-preview-page">
  <div class="editor-toolbar">
    <span class="editor-filepath" title={filePath}>{filePath}</span>
    <button class="md-edit-btn" onclick={onEdit} title="Edit file">
      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"></path></svg>
      Edit
    </button>
  </div>
  {#if loading}
    <div class="preview-loading">Loading preview...</div>
  {:else if error}
    <div class="preview-error">{error}</div>
  {:else}
    <div class="markdown-body">
      {#if frontmatter}
        <div class="md-frontmatter">
          {#each Object.entries(frontmatter) as [key, value]}
            <span class="md-fm-key">{key}:</span> <span class="md-fm-value">{value}</span>
          {/each}
        </div>
      {/if}
      {@html renderedHtml}
    </div>
  {/if}
</div>
