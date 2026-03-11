<script lang="ts">
  interface Props {
    x: number;
    y: number;
    onSplitTop: () => void;
    onSplitBottom: () => void;
    onSplitLeft: () => void;
    onSplitRight: () => void;
    onClose: () => void;
    onDismiss: () => void;
    canClose: boolean;
  }
  
  let { x, y, onSplitTop, onSplitBottom, onSplitLeft, onSplitRight, onClose, onDismiss, canClose }: Props = $props();
  
  function handleBackdropClick() {
    onDismiss();
  }
</script>

<div class="context-menu-backdrop" onclick={handleBackdropClick}></div>
<div class="context-menu" style:left="{x}px" style:top="{y}px">
  <div class="context-menu-section">
    <button class="context-menu-item" onclick={onSplitTop}>
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"></rect>
        <line x1="3" y1="12" x2="21" y2="12"></line>
        <line x1="12" y1="3" x2="12" y2="12" stroke-width="2"></line>
      </svg>
      <span>Split Top</span>
      <span class="context-menu-shortcut">⌘⇧↑</span>
    </button>
    <button class="context-menu-item" onclick={onSplitBottom}>
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"></rect>
        <line x1="3" y1="12" x2="21" y2="12"></line>
        <line x1="12" y1="12" x2="12" y2="21" stroke-width="2"></line>
      </svg>
      <span>Split Bottom</span>
      <span class="context-menu-shortcut">⌘⇧↓</span>
    </button>
    <button class="context-menu-item" onclick={onSplitLeft}>
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"></rect>
        <line x1="12" y1="3" x2="12" y2="21"></line>
        <line x1="3" y1="12" x2="12" y2="12" stroke-width="2"></line>
      </svg>
      <span>Split Left</span>
      <span class="context-menu-shortcut">⌘⇧←</span>
    </button>
    <button class="context-menu-item" onclick={onSplitRight}>
      <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"></rect>
        <line x1="12" y1="3" x2="12" y2="21"></line>
        <line x1="12" y1="12" x2="21" y2="12" stroke-width="2"></line>
      </svg>
      <span>Split Right</span>
      <span class="context-menu-shortcut">⌘⇧→</span>
    </button>
  </div>
  {#if canClose}
    <div class="context-menu-divider"></div>
    <div class="context-menu-section">
      <button class="context-menu-item danger" onclick={onClose}>
        <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
        <span>Close Pane</span>
        <span class="context-menu-shortcut">⌘W</span>
      </button>
    </div>
  {/if}
</div>

<style>
  .context-menu-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 999;
  }
  
  .context-menu {
    position: fixed;
    z-index: 1000;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    min-width: 200px;
    padding: 4px;
    animation: contextMenuFadeIn 0.1s ease;
  }
  
  @keyframes contextMenuFadeIn {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
  
  .context-menu-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  
  .context-menu-divider {
    height: 1px;
    background: var(--border-default);
    margin: 4px 0;
  }
  
  .context-menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 13px;
    cursor: pointer;
    border-radius: 4px;
    transition: all 0.15s ease;
    text-align: left;
    width: 100%;
  }
  
  .context-menu-item:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }
  
  .context-menu-item svg {
    flex-shrink: 0;
  }
  
  .context-menu-item span:first-of-type {
    flex: 1;
  }
  
  .context-menu-shortcut {
    font-size: 11px;
    color: var(--text-muted);
    opacity: 0.6;
  }
  
  .context-menu-item.danger:hover {
    background: rgba(255, 85, 85, 0.1);
    color: #ff5555;
  }
  
  .context-menu-item.danger:hover .context-menu-shortcut {
    color: #ff5555;
  }
</style>
