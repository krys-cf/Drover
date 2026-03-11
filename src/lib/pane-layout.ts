/**
 * Tmux-style pane layout system
 * Supports recursive splitting in horizontal and vertical directions
 */

export type PaneId = string;
export type LayoutDirection = 'horizontal' | 'vertical';

export interface PaneLeaf {
  type: 'leaf';
  id: PaneId;
  tabId: string;
}

export interface PaneContainer {
  type: 'container';
  id: PaneId;
  direction: LayoutDirection;
  children: PaneNode[];
  sizes: number[]; // percentage sizes for each child (sum to 100)
}

export type PaneNode = PaneLeaf | PaneContainer;

export interface PaneLayout {
  root: PaneNode;
  focusedPaneId: PaneId;
}

let paneCounter = 0;

/**
 * Deep-clone a pane tree so every node gets a fresh object reference.
 * This is critical for Svelte 5 $state reactivity which tracks by reference.
 */
export function deepCloneNode(node: PaneNode): PaneNode {
  if (node.type === 'leaf') {
    return { ...node };
  }
  return {
    ...node,
    children: node.children.map(deepCloneNode),
    sizes: [...node.sizes]
  };
}

export function generatePaneId(): PaneId {
  return `pane-${++paneCounter}`;
}

/**
 * Create initial single-pane layout
 */
export function createInitialLayout(tabId: string): PaneLayout {
  const paneId = generatePaneId();
  return {
    root: {
      type: 'leaf',
      id: paneId,
      tabId
    },
    focusedPaneId: paneId
  };
}

/**
 * Find a pane node by ID in the tree
 */
export function findPane(node: PaneNode, paneId: PaneId): PaneNode | null {
  if (node.id === paneId) return node;
  if (node.type === 'container') {
    for (const child of node.children) {
      const found = findPane(child, paneId);
      if (found) return found;
    }
  }
  return null;
}

/**
 * Find parent container of a pane
 */
export function findParent(
  node: PaneNode,
  paneId: PaneId,
  parent: PaneContainer | null = null
): PaneContainer | null {
  if (node.id === paneId) return parent;
  if (node.type === 'container') {
    for (const child of node.children) {
      const found = findParent(child, paneId, node);
      if (found) return found;
    }
  }
  return null;
}

/**
 * Split a pane in the specified direction
 * @param splitDirection - 'top', 'bottom', 'left', 'right' for directional splits
 */
export function splitPane(
  layout: PaneLayout,
  paneId: PaneId,
  splitDirection: 'top' | 'bottom' | 'left' | 'right',
  newTabId: string
): PaneLayout {
  // Deep-clone the entire tree so every node is a fresh reference
  const newRoot = deepCloneNode(layout.root);

  const pane = findPane(newRoot, paneId);
  if (!pane || pane.type !== 'leaf') return layout;

  const newPaneId = generatePaneId();
  const newPane: PaneLeaf = {
    type: 'leaf',
    id: newPaneId,
    tabId: newTabId
  };

  // Determine layout direction and child order based on split direction
  const isVerticalSplit = splitDirection === 'left' || splitDirection === 'right';
  const direction: LayoutDirection = isVerticalSplit ? 'vertical' : 'horizontal';
  const newPaneFirst = splitDirection === 'top' || splitDirection === 'left';
  const children = newPaneFirst ? [newPane, pane] : [pane, newPane];

  const container: PaneContainer = {
    type: 'container',
    id: generatePaneId(),
    direction,
    children,
    sizes: [50, 50]
  };

  // If root is the pane being split, the container becomes the new root
  if (newRoot.id === paneId) {
    return {
      root: container,
      focusedPaneId: newPaneId
    };
  }

  // Find parent in the cloned tree and replace the pane with the container
  const parent = findParent(newRoot, paneId);
  if (!parent) return layout;

  const index = parent.children.findIndex(c => c.id === paneId);
  if (index === -1) return layout;

  parent.children[index] = container;

  return {
    root: newRoot,
    focusedPaneId: newPaneId
  };
}

/**
 * Close a pane and remove it from the layout
 */
export function closePane(layout: PaneLayout, paneId: PaneId): PaneLayout | null {
  // Can't close the last pane
  if (layout.root.type === 'leaf' && layout.root.id === paneId) {
    return null;
  }

  // Deep-clone the entire tree so every node is a fresh reference
  let newRoot = deepCloneNode(layout.root);

  const parent = findParent(newRoot, paneId);
  if (!parent) return layout;

  // Remove the pane from parent's children
  const index = parent.children.findIndex(c => c.id === paneId);
  if (index === -1) return layout;

  parent.children.splice(index, 1);
  parent.sizes.splice(index, 1);

  // Redistribute sizes
  if (parent.children.length > 0) {
    const totalSize = parent.sizes.reduce((a, b) => a + b, 0);
    parent.sizes = parent.sizes.map(s => (s / totalSize) * 100);
  }

  // If parent now has only one child, collapse it
  if (parent.children.length === 1) {
    const grandparent = findParent(newRoot, parent.id);
    const onlyChild = parent.children[0];
    
    if (grandparent) {
      const parentIndex = grandparent.children.findIndex(c => c.id === parent.id);
      grandparent.children[parentIndex] = onlyChild;
    } else {
      // Parent is root, replace root with only child
      newRoot = onlyChild;
    }
  }

  // Update focused pane if we closed the focused one
  let newFocusedPaneId = layout.focusedPaneId;
  if (paneId === layout.focusedPaneId) {
    newFocusedPaneId = findFirstLeaf(newRoot)?.id || '';
  }

  return {
    root: newRoot,
    focusedPaneId: newFocusedPaneId
  };
}

/**
 * Find the first leaf pane in the tree
 */
export function findFirstLeaf(node: PaneNode): PaneLeaf | null {
  if (node.type === 'leaf') return node;
  if (node.children.length > 0) {
    return findFirstLeaf(node.children[0]);
  }
  return null;
}

/**
 * Get all leaf panes in the layout
 */
export function getAllLeaves(node: PaneNode): PaneLeaf[] {
  if (node.type === 'leaf') return [node];
  return node.children.flatMap(child => getAllLeaves(child));
}

/**
 * Navigate to adjacent pane (for keyboard navigation)
 */
export function navigatePane(
  layout: PaneLayout,
  direction: 'up' | 'down' | 'left' | 'right'
): PaneLayout {
  // TODO: Implement directional navigation based on pane positions
  // For now, just cycle through panes
  const leaves = getAllLeaves(layout.root);
  const currentIndex = leaves.findIndex(l => l.id === layout.focusedPaneId);
  if (currentIndex === -1) return layout;

  let nextIndex: number;
  if (direction === 'right' || direction === 'down') {
    nextIndex = (currentIndex + 1) % leaves.length;
  } else {
    nextIndex = (currentIndex - 1 + leaves.length) % leaves.length;
  }

  return {
    ...layout,
    focusedPaneId: leaves[nextIndex].id
  };
}

/**
 * Swap the tabIds of two leaf panes (for drag-to-reorder)
 */
export function swapPanes(layout: PaneLayout, paneId1: PaneId, paneId2: PaneId): PaneLayout {
  const newRoot = deepCloneNode(layout.root);
  const pane1 = findPane(newRoot, paneId1);
  const pane2 = findPane(newRoot, paneId2);
  if (!pane1 || !pane2 || pane1.type !== 'leaf' || pane2.type !== 'leaf') return layout;
  const tmp = pane1.tabId;
  pane1.tabId = pane2.tabId;
  pane2.tabId = tmp;
  return { root: newRoot, focusedPaneId: layout.focusedPaneId };
}

/**
 * Resize panes by adjusting sizes
 */
export function resizePane(
  layout: PaneLayout,
  paneId: PaneId,
  delta: number
): PaneLayout {
  const newRoot = deepCloneNode(layout.root);
  const parent = findParent(newRoot, paneId);
  if (!parent) return layout;

  const index = parent.children.findIndex(c => c.id === paneId);
  if (index === -1 || index === parent.children.length - 1) return layout;

  // Adjust this pane and the next one
  const newSize = Math.max(10, Math.min(90, parent.sizes[index] + delta));
  const diff = newSize - parent.sizes[index];
  
  parent.sizes[index] = newSize;
  parent.sizes[index + 1] = Math.max(10, parent.sizes[index + 1] - diff);

  return { root: newRoot, focusedPaneId: layout.focusedPaneId };
}
