// "View update" on the Device page reopens the update dialog, which lives at the app root. A DOM
// event keeps the two apart without a shared store.
const EVENT = 'kivori:update-dialog';

export function requestUpdateDialog(): void {
  window.dispatchEvent(new Event(EVENT));
}

export function onUpdateDialogRequest(handler: () => void): () => void {
  window.addEventListener(EVENT, handler);
  return () => window.removeEventListener(EVENT, handler);
}
