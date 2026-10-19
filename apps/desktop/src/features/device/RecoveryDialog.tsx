import type { ReactElement } from 'react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@kivori/ui/components/alert-dialog';
import { format, strings } from '@/lib/i18n/strings';

/**
 * Walks the user through putting a Kivori into download mode with the BOOT button, then asks the
 * native side to restore the bundled firmware. The native side finds the port itself.
 */
export function RecoveryDialog({
  open,
  onOpenChange,
  onRestore,
  target,
  disabled = false,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRestore: () => void;
  target?: string | undefined;
  disabled?: boolean;
}): ReactElement {
  const t = strings.recovery;
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t.title}</AlertDialogTitle>
          <AlertDialogDescription>{t.description}</AlertDialogDescription>
        </AlertDialogHeader>
        {target ? <p className="text-sm font-medium">{format(t.target, { target })}</p> : null}
        <ol className="space-y-2 text-sm">
          {t.steps.map((step, index) => (
            <li key={step} className="flex items-start gap-3">
              <span
                aria-hidden="true"
                className="flex size-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary"
              >
                {index + 1}
              </span>
              <span className="pt-0.5">{step}</span>
            </li>
          ))}
        </ol>
        <AlertDialogFooter>
          <AlertDialogCancel>{t.cancel}</AlertDialogCancel>
          <AlertDialogAction disabled={disabled} onClick={onRestore}>
            {t.confirm}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
