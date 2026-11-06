"""How `atm component add` and `component edit` describe what they moved (docs/layered-architecture.md §9, step 10)."""


def moved_text(label, from_label, assign, by_files):
    """What --from-label and --assign did, e.g. "6 tasks from the label atm-cli, 2 more by their files"."""
    parts = []
    if label:
        parts.append(f"{len(from_label)} task{'' if len(from_label) == 1 else 's'} from the label {label}")
    if assign:
        parts.append(f"{len(by_files)} {'more ' if label else ''}by their files")
    return ", ".join(parts)
