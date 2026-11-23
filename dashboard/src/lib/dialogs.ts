// The app-wide dialogs (About, Help, Connect your agents): above every tab and page, so not in the address (ATM-697,
// docs/tabs-and-navigation-design.md §4.4, §6.4): opening or closing one is no step in a tab's history. An address
// from before ("?about", an old link) still opens its dialog, and the flag is then taken out of it.
import { addressParams, formatAddress } from './address';

export const DIALOGS = ['about', 'help', 'agents'] as const;
export type DialogId = (typeof DIALOGS)[number];

/** The dialog an (older) address names, if any. */
export function dialogOf(address: string): DialogId | null {
  const params = addressParams(address);
  return DIALOGS.find((id) => params.has(id)) ?? null;
}

/** `address` with `dialog`'s flag (or none, for null), keeping the other parameters. */
export function withDialog(address: string, dialog: DialogId | null): string {
  const params = addressParams(address);
  for (const id of DIALOGS) params.delete(id);
  if (dialog) params.set(dialog, '');
  return formatAddress(params);
}
