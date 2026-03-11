<script lang="ts" module>
  import type { PaneNode, PaneLeaf, PaneContainer } from '$lib/pane-layout';
  
  export interface PaneTreeContext {
    focusedPaneId: string;
    getTab: (tabId: string) => any;
    onFocusPane: (paneId: string) => void;
    onResizeStart: (e: MouseEvent, index: number, container: PaneContainer) => void;
  }
</script>

<script lang="ts">
  import PaneTreeRenderer from './PaneTreeRenderer.svelte';
  import type { Snippet } from 'svelte';
  
  interface Props {
    node: PaneNode;
    context: PaneTreeContext;
    renderLeaf: Snippet<[{ leaf: PaneLeaf; tab: any; isFocused: boolean }]>;
  }
  
  let { node, context, renderLeaf }: Props = $props();
  
  function isLeaf(n: PaneNode): n is PaneLeaf {
    return n.type === 'leaf';
  }
  
  function isContainer(n: PaneNode): n is PaneContainer {
    return n.type === 'container';
  }
</script>

{#if isLeaf(node)}
  {@const tab = context.getTab(node.tabId)}
  {#if tab}
    {@render renderLeaf({ leaf: node, tab, isFocused: node.id === context.focusedPaneId })}
  {/if}
{:else if isContainer(node)}
  <div 
    class="pane-container" 
    class:horizontal={node.direction === 'horizontal'}
    class:vertical={node.direction === 'vertical'}
    data-container-id={node.id}
  >
    {#each node.children as child, i (child.id)}
      <div class="pane-wrapper" style="flex: {node.sizes[i] || (100 / node.children.length)}">
        <PaneTreeRenderer 
          node={child}
          {context}
          {renderLeaf}
        />
      </div>
      {#if i < node.children.length - 1}
        <div 
          class="split-divider" 
          onmousedown={(e) => context.onResizeStart(e, i, node)}
          role="separator"
          aria-orientation={node.direction === 'horizontal' ? 'horizontal' : 'vertical'}
        ></div>
      {/if}
    {/each}
  </div>
{/if}
