<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getFileColor } from '$lib/theme';
  import type { FileEntry } from '$lib/types';
  import { tick } from 'svelte';

  let {
    root = $bindable('~'),
    open = $bindable(false),
    showHidden = $bindable(false),
    sshTarget = '',
    onOpenFile,
    onPreviewFile,
    onNavigate,
    getInitialCwd,
  }: {
    root: string;
    open: boolean;
    showHidden: boolean;
    sshTarget: string;
    onOpenFile: (entry: FileEntry) => void;
    onPreviewFile: (entry: FileEntry) => void;
    onNavigate: (path: string) => void;
    getInitialCwd: () => Promise<string | null>;
  } = $props();

  const MD_EXTS = new Set(['md', 'mdx']);

  function isMarkdownFile(name: string): boolean {
    const ext = name.split('.').pop()?.toLowerCase() || '';
    return MD_EXTS.has(ext);
  }

  let entries: FileEntry[] = $state([]);
  let loading = $state(false);

  let ctxMenu = $state<{ x: number; y: number; entry: FileEntry | null; parentPath: string } | null>(null);
  let renaming = $state<{ entry: FileEntry; value: string } | null>(null);
  let creating = $state<{ parentPath: string; isDir: boolean; value: string; depth: number; afterEntry?: FileEntry } | null>(null);
  let clipboard = $state<{ path: string; name: string; cut: boolean } | null>(null);
  let selectedEntry = $state<FileEntry | null>(null);

  const SYSTEM_HIDDEN = new Set([
    '.DS_Store', '.Trash', '.Spotlight-V100', '.fseventsd',
    '.CFUserTextEncoding', '.localized',
    '.git', '.svn', '.hg',
    '__pycache__', '.pytest_cache', '.mypy_cache', '.ruff_cache',
    '.tsbuildinfo',
  ]);

  function isSystemHidden(entry: FileEntry): boolean {
    if (!entry.is_hidden) return false;
    return SYSTEM_HIDDEN.has(entry.name);
  }

  async function doOpen() {
    const cwd = await getInitialCwd();
    if (cwd) root = cwd;
    await loadRoot();
  }

  $effect(() => {
    if (open) {
      doOpen();
    }
  });

  async function listDir(path: string) {
    if (sshTarget) {
      return await invoke<{ name: string; is_dir: boolean; is_hidden: boolean; path: string }[]>('list_remote_directory', { sshTarget, path });
    }
    return await invoke<{ name: string; is_dir: boolean; is_hidden: boolean; path: string }[]>('list_directory', { path });
  }

  async function loadRoot() {
    loading = true;
    try {
      const result = await listDir(root);
      entries = result.map(e => ({ ...e, children: undefined, expanded: false, loading: false }));
    } catch (e) {
      console.error('Failed to list directory:', e);
      entries = [];
    } finally {
      loading = false;
    }
  }

  async function reloadParent(parentPath: string) {
    if (parentPath === root) {
      await loadRoot();
    } else {
      const entry = findEntryByPath(entries, parentPath);
      if (entry && entry.is_dir) {
        try {
          const children = await listDir(entry.path);
          entry.children = children.map(e => ({ ...e, children: undefined, expanded: false, loading: false }));
          entry.expanded = true;
          entries = [...entries];
        } catch (e) {
          console.error('Failed to reload directory:', e);
        }
      }
    }
  }

  function findEntryByPath(items: FileEntry[], path: string): FileEntry | null {
    for (const e of items) {
      if (e.path === path) return e;
      if (e.children) {
        const found = findEntryByPath(e.children, path);
        if (found) return found;
      }
    }
    return null;
  }

  function getParentPath(entryPath: string): string {
    if (entryPath.startsWith('~')) {
      const parts = entryPath.split('/');
      parts.pop();
      return parts.length <= 1 ? '~' : parts.join('/');
    }
    const parts = entryPath.split('/');
    parts.pop();
    return parts.join('/') || '/';
  }

  async function toggleDirectory(entry: FileEntry) {
    if (!entry.is_dir) return;
    if (entry.expanded) {
      entry.expanded = false;
      entries = [...entries];
      return;
    }
    entry.loading = true;
    entries = [...entries];
    try {
      const children = await listDir(entry.path);
      entry.children = children.map(e => ({ ...e, children: undefined, expanded: false, loading: false }));
      entry.expanded = true;
    } catch (e) {
      console.error('Failed to list directory:', e);
      entry.children = [];
      entry.expanded = true;
    } finally {
      entry.loading = false;
      entries = [...entries];
    }
  }

  async function navigateUp() {
    if (root === '/' || root === '~') {
      if (root === '~') {
        root = '/';
      }
    } else {
      const parts = root.split('/');
      parts.pop();
      root = parts.join('/') || '/';
    }
    await loadRoot();
    onNavigate(root);
  }

  async function navigateInto(entry: FileEntry) {
    if (!entry.is_dir) return;
    root = entry.path;
    entries = [];
    await loadRoot();
    onNavigate(root);
  }

  function handleEntryClick(entry: FileEntry) {
    selectedEntry = entry;
    if (entry.is_dir) {
      toggleDirectory(entry);
    } else {
      onOpenFile(entry);
    }
  }

  function handleEntryDblClick(entry: FileEntry) {
    if (entry.is_dir) {
      navigateInto(entry);
    }
  }

  function handleContextMenu(e: MouseEvent, entry: FileEntry | null, parentPath: string) {
    e.preventDefault();
    e.stopPropagation();
    if (entry) selectedEntry = entry;
    ctxMenu = { x: e.clientX, y: e.clientY, entry, parentPath };
  }

  function closeContextMenu() {
    ctxMenu = null;
  }

  function startRename(entry: FileEntry) {
    closeContextMenu();
    renaming = { entry, value: entry.name };
    tick().then(() => {
      const input = document.querySelector('.fe-rename-input') as HTMLInputElement;
      if (input) {
        input.focus();
        const dot = entry.name.lastIndexOf('.');
        input.setSelectionRange(0, dot > 0 && !entry.is_dir ? dot : entry.name.length);
      }
    });
  }

  async function commitRename() {
    if (!renaming) return;
    const { entry, value } = renaming;
    const trimmed = value.trim();
    if (!trimmed || trimmed === entry.name) {
      renaming = null;
      return;
    }
    const parentPath = getParentPath(entry.path);
    const newPath = parentPath.endsWith('/') ? parentPath + trimmed : parentPath + '/' + trimmed;
    try {
      await invoke('rename_path', { oldPath: entry.path, newPath });
      renaming = null;
      await reloadParent(parentPath);
    } catch (e) {
      console.error('Rename failed:', e);
      renaming = null;
    }
  }

  function startCreate(isDir: boolean, parentPath: string, depth: number, afterEntry?: FileEntry) {
    console.log('[startCreate]', { isDir, parentPath, depth, root, ctxEntry: ctxMenu?.entry?.path, ctxParent: ctxMenu?.parentPath });
    closeContextMenu();
    creating = { parentPath, isDir, value: '', depth, afterEntry };
    tick().then(() => {
      const input = document.querySelector('.fe-create-input') as HTMLInputElement;
      input?.focus();
    });
  }

  async function commitCreate() {
    if (!creating) return;
    const { parentPath, isDir, value } = creating;
    const trimmed = value.trim();
    if (!trimmed) {
      creating = null;
      return;
    }
    const newPath = parentPath.endsWith('/') ? parentPath + trimmed : parentPath + '/' + trimmed;
    try {
      if (isDir) {
        await invoke('create_directory', { path: newPath });
      } else {
        await invoke('create_file', { path: newPath });
      }
      creating = null;
      await reloadParent(parentPath);
    } catch (e) {
      console.error('Create failed:', e);
      creating = null;
    }
  }

  async function deleteEntry(entry: FileEntry) {
    closeContextMenu();
    try {
      await invoke('delete_path', { path: entry.path });
      if (selectedEntry === entry) selectedEntry = null;
      const parentPath = getParentPath(entry.path);
      await reloadParent(parentPath);
    } catch (e) {
      console.error('Delete failed:', e);
    }
  }

  function copyEntry(entry: FileEntry, cut: boolean) {
    closeContextMenu();
    clipboard = { path: entry.path, name: entry.name, cut };
  }

  async function pasteEntry(destDir: string) {
    closeContextMenu();
    if (!clipboard) return;
    const destPath = destDir.endsWith('/') ? destDir + clipboard.name : destDir + '/' + clipboard.name;
    try {
      if (clipboard.cut) {
        await invoke('rename_path', { oldPath: clipboard.path, newPath: destPath });
        clipboard = null;
      } else {
        await invoke('copy_path', { source: clipboard.path, dest: destPath });
      }
      await reloadParent(destDir);
    } catch (e) {
      console.error('Paste failed:', e);
    }
  }

  async function copyPath(entry: FileEntry) {
    closeContextMenu();
    try {
      await navigator.clipboard.writeText(entry.path);
    } catch (e) {
      console.error('Copy path failed:', e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!selectedEntry) return;
    const meta = e.metaKey || e.ctrlKey;
    if (meta && e.key === 'c') {
      e.preventDefault();
      copyEntry(selectedEntry, false);
    } else if (meta && e.key === 'x') {
      e.preventDefault();
      copyEntry(selectedEntry, true);
    } else if (meta && e.key === 'v') {
      e.preventDefault();
      const destDir = selectedEntry.is_dir ? selectedEntry.path : getParentPath(selectedEntry.path);
      pasteEntry(destDir);
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      if (!renaming && !creating) {
        e.preventDefault();
        deleteEntry(selectedEntry);
      }
    } else if (e.key === 'F2') {
      e.preventDefault();
      startRename(selectedEntry);
    }
  }
</script>

{#snippet createInput(depth: number)}
  {#if creating}
    <div class="fe-entry fe-creating" style="padding-left: {12 + depth * 16}px">
      <span class="fe-icon">
        {#if creating.isDir}
          <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
        {:else}
          <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>
        {/if}
      </span>
      <input
        class="fe-create-input"
        type="text"
        bind:value={creating.value}
        onkeydown={(e) => { if (e.key === 'Enter') commitCreate(); else if (e.key === 'Escape') creating = null; }}
        onblur={commitCreate}
        placeholder={creating.isDir ? 'folder name' : 'file name'}
      />
    </div>
  {/if}
{/snippet}

{#snippet fileTree(items: FileEntry[], depth: number)}
  {#if creating && creating.parentPath === root && creating.depth === depth && !creating.afterEntry}
    {@render createInput(depth)}
  {/if}
  {#each items as entry}
    {#if (!isSystemHidden(entry) || showHidden) && !entry.name.endsWith('.save')}
      <div
        class="fe-entry"
        class:fe-dir={entry.is_dir}
        class:fe-file={!entry.is_dir}
        class:fe-hidden={entry.is_hidden}
        class:fe-selected={selectedEntry === entry}
        class:fe-cut={clipboard?.cut && clipboard.path === entry.path}
        style="padding-left: {12 + depth * 16}px"
        onclick={() => handleEntryClick(entry)}
        ondblclick={() => handleEntryDblClick(entry)}
        oncontextmenu={(e) => handleContextMenu(e, entry, entry.is_dir ? entry.path : getParentPath(entry.path))}
        onkeydown={(e) => { if (e.key === 'Enter') handleEntryClick(entry); }}
        role="treeitem"
        aria-selected={selectedEntry === entry}
        tabindex="0"
      >
        <span class="fe-icon">
          {#if entry.is_dir}
            {#if entry.loading}
              <svg class="fe-spinner" viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none"><circle cx="12" cy="12" r="10" stroke-dasharray="31.4" stroke-dashoffset="10"></circle></svg>
            {:else if entry.expanded}
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
            {:else}
              <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
            {/if}
          {:else}
            {@const fc = getFileColor(entry.name)}
            <svg viewBox="0 0 24 24" width="12" height="12" stroke={fc || 'currentColor'} stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>
          {/if}
        </span>
        {#if renaming && renaming.entry === entry}
          <input
            class="fe-rename-input"
            type="text"
            bind:value={renaming.value}
            onkeydown={(e) => { if (e.key === 'Enter') commitRename(); else if (e.key === 'Escape') renaming = null; }}
            onblur={commitRename}
          />
        {:else}
          <span class="fe-name" style={!entry.is_dir && getFileColor(entry.name) ? `color: ${getFileColor(entry.name)}` : ''}>{entry.name}</span>
        {/if}
      </div>
      {#if creating && creating.afterEntry === entry}
        {@render createInput(depth)}
      {/if}
      {#if entry.is_dir && entry.expanded && entry.children}
        {#if creating && creating.parentPath === entry.path}
          {@render createInput(depth + 1)}
        {/if}
        {@render fileTree(entry.children, depth + 1)}
      {/if}
    {/if}
  {/each}
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<aside class="file-explorer" role="none" onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown} oncontextmenu={(e) => handleContextMenu(e, null, root)}>
  <div class="fe-header">
    <span class="fe-header-title">{sshTarget ? `Remote: ${sshTarget}` : 'Explorer'}</span>
    <div class="fe-header-actions">
      <button
        class="fe-header-btn"
        onclick={() => startCreate(false, root, 0)}
        title="New File"
        aria-label="New File"
      >
        <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
      </button>
      <button
        class="fe-header-btn"
        onclick={() => startCreate(true, root, 0)}
        title="New Folder"
        aria-label="New Folder"
      >
        <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
      </button>
      <button
        class="fe-header-btn"
        class:active={showHidden}
        onclick={() => { showHidden = !showHidden; }}
        title={showHidden ? 'Hide hidden files' : 'Show hidden files'}
        aria-label="Toggle hidden files"
      >
        <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path></svg>
      </button>
      <button class="fe-header-btn" onclick={loadRoot} title="Refresh" aria-label="Refresh">
        <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"></polyline><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"></path></svg>
      </button>
    </div>
  </div>
  <div class="fe-breadcrumb">
    <button class="fe-breadcrumb-up" onclick={navigateUp} title="Go up" aria-label="Go up">
      <svg viewBox="0 0 24 24" width="12" height="12" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"></polyline></svg>
    </button>
    <span class="fe-breadcrumb-path" title={root}>{root}</span>
  </div>
  <div class="fe-tree" role="tree">
    {#if loading}
      <div class="fe-loading">Loading...</div>
    {:else if entries.length === 0}
      {@render createInput(0)}
      <div class="fe-empty">Empty directory</div>
    {:else}
      {@render fileTree(entries, 0)}
    {/if}
  </div>
</aside>

{#if ctxMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="context-menu-backdrop" onclick={closeContextMenu} oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}></div>
  <div class="pane-context-menu" style="left: {ctxMenu.x}px; top: {ctxMenu.y}px;" role="menu">
    {#if ctxMenu.entry}
      <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) onOpenFile(ctxMenu.entry); closeContextMenu(); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>
        Open
      </button>
      {#if !ctxMenu.entry.is_dir && isMarkdownFile(ctxMenu.entry.name)}
        <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) onPreviewFile(ctxMenu.entry); closeContextMenu(); }} role="menuitem">
          <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path><circle cx="12" cy="12" r="3"></circle></svg>
          Preview
        </button>
      {/if}
      <div class="pane-context-separator"></div>
    {/if}
    <button class="pane-context-item" onclick={() => startCreate(false, ctxMenu?.entry?.is_dir ? ctxMenu.entry.path : ctxMenu?.parentPath || root, 0)} role="menuitem">
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
      New File
    </button>
    <button class="pane-context-item" onclick={() => startCreate(true, ctxMenu?.entry?.is_dir ? ctxMenu.entry.path : ctxMenu?.parentPath || root, 0)} role="menuitem">
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
      New Folder
    </button>
    {#if ctxMenu.entry}
      <div class="pane-context-separator"></div>
      <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) startRename(ctxMenu.entry); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"></path></svg>
        Rename
      </button>
      <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) copyEntry(ctxMenu.entry, false); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><rect width="14" height="14" x="8" y="8" rx="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg>
        Copy
      </button>
      <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) copyEntry(ctxMenu.entry, true); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><rect width="14" height="14" x="8" y="8" rx="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path><line x1="2" y1="2" x2="22" y2="22" stroke-dasharray="3 3"></line></svg>
        Cut
      </button>
    {/if}
    {#if clipboard}
      <button class="pane-context-item" onclick={() => pasteEntry(ctxMenu?.entry?.is_dir ? ctxMenu.entry.path : ctxMenu?.parentPath || root)} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"></path><rect width="8" height="4" x="8" y="2" rx="1"></rect></svg>
        Paste
      </button>
    {/if}
    {#if ctxMenu.entry}
      <div class="pane-context-separator"></div>
      <button class="pane-context-item" onclick={() => { if (ctxMenu?.entry) copyPath(ctxMenu.entry); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path></svg>
        Copy Path
      </button>
      <div class="pane-context-separator"></div>
      <button class="pane-context-item danger" onclick={() => { if (ctxMenu?.entry) deleteEntry(ctxMenu.entry); }} role="menuitem">
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
        Delete
      </button>
    {/if}
  </div>
{/if}
