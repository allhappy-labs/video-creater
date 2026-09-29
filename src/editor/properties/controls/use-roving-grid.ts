import { useRef, useState, type KeyboardEvent } from "react";

interface RovingItemProps {
  readonly tabIndex: number;
  readonly ref: (element: HTMLButtonElement | null) => void;
  onFocus(): void;
  onKeyDown(event: KeyboardEvent<HTMLButtonElement>): void;
}

function nextIndex(key: string, index: number, count: number, columns: number): number | null {
  switch (key) {
    case "ArrowRight":
      return Math.min(count - 1, index + 1);
    case "ArrowLeft":
      return Math.max(0, index - 1);
    case "ArrowDown":
      return index + columns < count ? index + columns : index;
    case "ArrowUp":
      return index - columns >= 0 ? index - columns : index;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

/**
 * Roving tab stop for a grid of toggle buttons: one Tab stop (the focused, else the selected,
 * else the first item), arrow keys move by one item or one row, Home and End jump to the ends.
 * Focus moves without selecting; Enter or Space presses the focused button.
 */
export function useRovingGrid(count: number, selectedIndex: number, columns: number) {
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const [focusedIndex, setFocusedIndex] = useState<number | null>(null);
  const stop = focusedIndex !== null && focusedIndex < count ? focusedIndex : Math.max(0, selectedIndex);

  return (index: number): RovingItemProps => ({
    tabIndex: index === stop ? 0 : -1,
    ref: (element) => {
      items.current[index] = element;
    },
    onFocus: () => setFocusedIndex(index),
    onKeyDown: (event) => {
      const next = nextIndex(event.key, index, count, Math.max(1, columns));
      if (next === null) return;
      event.preventDefault();
      items.current[next]?.focus();
    },
  });
}
