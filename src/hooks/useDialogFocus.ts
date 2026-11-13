import { useEffect, useRef } from "react";

// Shared registration prevents an underlying dialog handling the same key event.
const overlays: HTMLDivElement[] = [];
const focusableSelector = "button, a[href], input, select, textarea, summary, [tabindex]";

function isTabbable(element: HTMLElement, panel: HTMLElement) {
  if (element.tabIndex < 0 || element.matches(':disabled, input[type="hidden"]')) return false;
  if (element.closest('[hidden], [inert], [aria-hidden="true"]')) return false;
  // A child's computed display does not reveal display:none on its ancestors.
  for (let current: HTMLElement | null = element; current; current = current.parentElement) {
    if (current instanceof HTMLDetailsElement && !current.open) {
      const summary = current.querySelector(":scope > summary");
      if (!summary?.contains(element)) return false;
    }
    const style = getComputedStyle(current);
    if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse") return false;
    if (current === panel) break;
  }
  return true;
}

export function useDialogFocus({ open, onClose, busy = false, modal = true }: {
  open: boolean;
  onClose: () => void;
  busy?: boolean;
  modal?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const latest = useRef({ onClose, busy });
  latest.current = { onClose, busy };

  useEffect(() => {
    const panel = ref.current;
    if (!open || !panel) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    // Child effects may register first when nested dialogs mount together.
    const descendantIndex = overlays.findIndex((overlay) => panel.contains(overlay));
    if (descendantIndex < 0) overlays.push(panel);
    else overlays.splice(descendantIndex, 0, panel);
    const isTop = () => overlays[overlays.length - 1] === panel;
    const controls = () => Array.from(panel.querySelectorAll<HTMLElement>(focusableSelector))
      .filter((element) => isTabbable(element, panel))
      .sort((left, right) => {
        // Positive tabindex precedes the normal document-order tab sequence.
        const leftOrder = left.tabIndex > 0 ? left.tabIndex : Infinity;
        const rightOrder = right.tabIndex > 0 ? right.tabIndex : Infinity;
        if (leftOrder !== rightOrder) return leftOrder - rightOrder;
        return left.compareDocumentPosition(right) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1;
      });
    const focusFirst = () => {
      const elements = controls();
      (elements.find((element) => element.hasAttribute("data-autofocus")) ?? elements[0] ?? panel).focus();
    };
    if (modal && isTop() && !panel.contains(document.activeElement)) focusFirst();

    const onKey = (event: KeyboardEvent) => {
      if (!isTop() || (!modal && !panel.contains(document.activeElement))) return;
      if (event.isComposing || event.keyCode === 229) return;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (!latest.current.busy) latest.current.onClose();
      }
      if (event.key !== "Tab" || !modal) return;
      const elements = controls();
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (!first) { event.preventDefault(); panel.focus(); return; }
      if (event.shiftKey && (document.activeElement === first || document.activeElement === panel)) {
        event.preventDefault(); last?.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !panel.contains(document.activeElement))) {
        event.preventDefault(); first.focus();
      }
    };
    const onFocus = (event: FocusEvent) => {
      if (modal && isTop() && !panel.contains(event.target as Node)) focusFirst();
    };
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocus);
    return () => {
      const wasTop = isTop();
      const index = overlays.indexOf(panel);
      if (index >= 0) overlays.splice(index, 1);
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocus);
      if (modal && wasTop && previous?.isConnected) previous.focus();
    };
  }, [open, modal]);
  return ref;
}
