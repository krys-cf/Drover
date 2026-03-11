<script lang="ts">
  import type { PaneNode, PaneLeaf, PaneContainer } from '$lib/pane-layout';
  import PaneLayout from './PaneLayout.svelte';
  import PaneContextMenu from './PaneContextMenu.svelte';
  
  interface Props {
    node: PaneNode;
    focusedPaneId: string;
    onFocusPane: (paneId: string) => void;
    onSplitPane: (paneId: string, direction: 'top' | 'bottom' | 'left' | 'right') => void;
    onClosePane: (paneId: string) => void;
    renderPane: (paneLeaf: PaneLeaf, isFocused: boolean) => any;
    canClosePane: (paneId: string) => boolean;
  }
  
  let { node, focusedPaneId, onFocusPane, onSplitPane, onClosePane, renderPane, canClosePane }: Props = $props();
  
  let contextMenu = $state<{ paneId: string; x: number; y: number } | null>(null);
  
  function isLeaf(n: PaneNode): n is PaneLeaf {
    return n.type === 'leaf';
  }
  
  function isContainer(n: PaneNode): n is PaneContainer {
    return n.type === 'container';
  }
  
  function handleContextMenu(e: MouseEvent, paneId: string) {
    e.preventDefault();
    e.stopPropagation();
    contextMenu = { paneId, x: e.clientX, y: e.clientY };
  }
  
  function dismissContextMenu() {
    contextMenu = null;
  }
</script>

{#if isLeaf(node)}
  <div 
    class="pane-leaf" 
    class:focused={node.id === focusedPaneId}
    onclick={() => onFocusPane(node.id)}
    oncontextmenu={(e) => handleContextMenu(e, node.id)}
  >
    <div class="pane-content">
      {@render renderPane(node, node.id === focusedPaneId)}
    </div>
  </div>
{:else if isContainer(node)}
  <div 
    class="pane-container" 
    class:horizontal={node.direction === 'horizontal'}
    class:vertical={node.direction === 'vertical'}
  >
    {#each node.children as child, i (child.id)}
      <div 
        class="pane-child" 
        style:flex={node.sizes[i] || 50}
      >
        <PaneLayout 
          node={child}
          {focusedPaneId}
          {onFocusPane}
          {onSplitPane}
          {onClosePane}
          {renderPane}
          {canClosePane}
        />
      </div>
      {#if i < node.children.length - 1}
        <div class="pane-divider"></div>
      {/if}
    {/each}
  </div>
{/if}

{#if contextMenu}
  <PaneContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    onSplitTop={() => { onSplitPane(contextMenu!.paneId, 'top'); dismissContextMenu(); }}
    onSplitBottom={() => { onSplitPane(contextMenu!.paneId, 'bottom'); dismissContextMenu(); }}
    onSplitLeft={() => { onSplitPane(contextMenu!.paneId, 'left'); dismissContextMenu(); }}
    onSplitRight={() => { onSplitPane(contextMenu!.paneId, 'right'); dismissContextMenu(); }}
    onClose={() => { onClosePane(contextMenu!.paneId); dismissContextMenu(); }}
    onDismiss={dismissContextMenu}
    canClose={canClosePane(contextMenu!.paneId)}
  />
{/if}

<style>
  .pane-leaf {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    flex: 1;
    background: var(--bg-primary);
    border: 2px solid transparent;
    transition: border-color 0.15s ease;
  }
  
  .pane-leaf.focused {
    border-color: var(--border-focus);
  }
  
  
  .pane-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  
  .pane-container {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  
  .pane-container.horizontal {
    flex-direction: column;
  }
  
  .pane-container.vertical {
    flex-direction: row;
  }
  
  .pane-child {
    display: flex;
    min-width: 0;
    min-height: 0;
  }
  
  .pane-divider {
    flex-shrink: 0;
    background: var(--border-default);
  }
  
  .pane-container.horizontal .pane-divider {
    height: 1px;
    width: 100%;
  }
  
  .pane-container.vertical .pane-divider {
    width: 1px;
    height: 100%;
  }
</style>
