<script lang="ts">
  import { tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { highlightLines } from '$lib/highlight';
  import { detectLang } from '$lib/theme';

  let {
    filePath = $bindable(''),
    fileName = $bindable(''),
    content = $bindable(''),
    dirty = $bindable(false),
    saveStatus = $bindable<'' | 'saving' | 'saved' | 'error'>(''),
    lang = $bindable('text'),
    loading = $bindable(false),
    error = $bindable(''),
    sshTarget = '',
    onClose,
  }: {
    filePath: string;
    fileName: string;
    content: string;
    dirty: boolean;
    saveStatus: '' | 'saving' | 'saved' | 'error';
    lang: string;
    loading: boolean;
    error: string;
    sshTarget: string;
    onClose: () => void;
  } = $props();

  let isRemote = $derived(!!sshTarget);

  let editorEl: HTMLDivElement | null = null;
  let editorGutterEl: HTMLDivElement | null = null;
  let highlightedHtml = $state('');
  let originalContent = '';

  function highlightCode(code: string, l: string) {
    try {
      highlightedHtml = highlightLines(code, l);
    } catch {
      highlightedHtml = '';
    }
  }

  let highlightTimer: ReturnType<typeof setTimeout> | null = null;
  function scheduleHighlight() {
    if (highlightTimer) clearTimeout(highlightTimer);
    highlightTimer = setTimeout(() => {
      highlightCode(content, lang);
      applyHighlightToEditor();
    }, 300);
  }

  function getCursorOffset(): number {
    if (!editorEl) return 0;
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || !editorEl.contains(sel.anchorNode)) return 0;
    const range = document.createRange();
    range.setStart(editorEl, 0);
    range.setEnd(sel.anchorNode!, sel.anchorOffset);
    const frag = range.cloneContents();
    const tmp = document.createElement('div');
    tmp.style.whiteSpace = 'pre';
    tmp.appendChild(frag);
    document.body.appendChild(tmp);
    const text = tmp.innerText;
    document.body.removeChild(tmp);
    return text.length;
  }

  function applyHighlightToEditor() {
    if (!editorEl) return;
    const hasFocus = document.activeElement === editorEl;
    const offset = hasFocus ? getCursorOffset() : 0;
    if (highlightedHtml) {
      editorEl.innerHTML = highlightedHtml;
    } else {
      editorEl.textContent = content;
    }
    if (hasFocus && offset > 0) {
      restoreCursor(offset);
    }
  }

  function restoreCursor(targetOffset: number) {
    if (!editorEl) return;
    let pos = 0;
    let lastLineEnded = false;

    function walk(node: Node): boolean {
      if (node.nodeType === Node.TEXT_NODE) {
        const len = node.textContent?.length || 0;
        if (pos + len >= targetOffset) {
          const sel = window.getSelection();
          const range = document.createRange();
          range.setStart(node, targetOffset - pos);
          range.collapse(true);
          sel?.removeAllRanges();
          sel?.addRange(range);
          return true;
        }
        pos += len;
        lastLineEnded = false;
        return false;
      }
      if (node.nodeType === Node.ELEMENT_NODE) {
        const el = node as HTMLElement;
        const isLine = el.classList?.contains('line');
        if (isLine && lastLineEnded) {
          pos++;
          if (pos >= targetOffset) {
            const firstText = el.querySelector('*')?.firstChild || el.firstChild;
            if (firstText) {
              const sel = window.getSelection();
              const range = document.createRange();
              range.setStart(firstText, 0);
              range.collapse(true);
              sel?.removeAllRanges();
              sel?.addRange(range);
              return true;
            }
          }
        }
        for (const child of node.childNodes) {
          if (walk(child)) return true;
        }
        if (isLine) lastLineEnded = true;
      }
      return false;
    }

    walk(editorEl);
  }

  export async function openFile(path: string, name: string) {
    filePath = path;
    fileName = name;
    content = '';
    originalContent = '';
    error = '';
    dirty = false;
    saveStatus = '';
    highlightedHtml = '';
    loading = true;
    lang = detectLang(name);
    try {
      const fileContent: string = sshTarget
        ? await invoke('read_remote_file_contents', { sshTarget, path })
        : await invoke('read_file_contents', { path });
      content = fileContent;
      originalContent = fileContent;
      highlightCode(fileContent, lang);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
      await tick();
      if (editorEl) {
        applyHighlightToEditor();
        editorEl.focus();
      }
    }
  }

  export async function saveFile() {
    if (!filePath || !dirty) return;
    saveStatus = 'saving';
    try {
      if (isRemote) {
        await invoke('write_remote_file_contents', { sshTarget, path: filePath, contents: content });
      } else {
        await invoke('write_file_contents', { path: filePath, contents: content });
      }
      originalContent = content;
      dirty = false;
      saveStatus = 'saved';
      setTimeout(() => { if (saveStatus === 'saved') saveStatus = ''; }, 2000);
    } catch (e) {
      error = String(e);
      saveStatus = 'error';
    }
  }

  function handleEditorInput() {
    if (!editorEl) return;
    content = editorEl.innerText;
    dirty = content !== originalContent;
    scheduleHighlight();
  }

  function handleEditorKeydown(e: KeyboardEvent) {
    if (e.key === 's' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      saveFile();
      return;
    }
    if (e.key === 'Tab') {
      e.preventDefault();
      document.execCommand('insertText', false, '  ');
      handleEditorInput();
    }
  }

  function syncEditorScroll() {
    if (editorEl && editorGutterEl) {
      editorGutterEl.scrollTop = editorEl.scrollTop;
    }
  }
</script>

<!-- Titlebar portion rendered by parent; this is just the editor content area -->
<div class="editor-page">
  <div class="editor-toolbar">
    <span class="editor-filepath" title={filePath}>{filePath}</span>
  </div>
  {#if loading}
    <div class="editor-loading">Loading file...</div>
  {:else if error}
    <div class="editor-error">{error}</div>
  {:else}
    <div class="editor-content">
      <div class="editor-gutter" bind:this={editorGutterEl}>
        {#each content.split('\n') as _, i}
          <div class="editor-line-number">{i + 1}</div>
        {/each}
      </div>
      <div
        class="editor-editable"
        bind:this={editorEl}
        contenteditable="true"
        role="textbox"
        aria-multiline="true"
        tabindex="0"
        spellcheck="false"
        oninput={handleEditorInput}
        onkeydown={handleEditorKeydown}
        onscroll={syncEditorScroll}
      ></div>
    </div>
  {/if}
</div>
