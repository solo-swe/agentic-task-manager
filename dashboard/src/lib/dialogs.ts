// The open dialog (About, Help) is a flag in the address ("?about"), so a dialog can be linked and Back closes it.
import { addressParams, formatAddress } from './address';

export const DIALOGS = ['about', 'help'] as const;
export type DialogId = (typeof DIALOGS)[number];

/** The dialog an address opens, if any. */
export function dialogOf(address: string): DialogId | null {
  const params = addressParams(address);
  return DIALOGS.find((id) => params.has(id)) ?? null;
}

/** `address` with `dialog` open (or none, for null), keeping the other parameters. */
export function withDialog(address: string, dialog: DialogId | null): string {
  const params = addressParams(address);
  for (const id of DIALOGS) params.delete(id);
  if (dialog) params.set(dialog, '');
  return formatAddress(params);
}
