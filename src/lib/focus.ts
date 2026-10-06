// Shared dialog keyboard handling; restore focus to the invoking control.
export function dialogFocus(node: HTMLElement) {
  const previous = document.activeElement as HTMLElement | null;
  const controls = () => [...node.querySelectorAll<HTMLElement>('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex="0"]')];
  (controls()[0] ?? node).focus();
  const keydown = (event: KeyboardEvent) => {
    if (event.key !== "Tab") return;
    const items = controls();
    const first = items[0] ?? node;
    const last = items.at(-1) ?? node;
    if (event.shiftKey && (document.activeElement === first || document.activeElement === node)) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus();
    }
  };
  node.addEventListener("keydown", keydown);
  return { destroy() { node.removeEventListener("keydown", keydown); previous?.focus(); } };
}
