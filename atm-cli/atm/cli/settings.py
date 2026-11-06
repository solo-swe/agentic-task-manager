"""`atm project logo`: the project's logo, through atm-lib. Setting the store itself up (`atm db init`: the merge
driver, the dashboard's list) is boot's, the one place that knows the store."""

from ..common.core import out, to_json
from ..common.errors import Invalid
from ..lib.settings import get_logo, set_logo, unset_logo


def logo_action(project, action, path, as_json):
    """Print, set or unset the logo (`atm project logo`)."""
    if action is None:
        if path is not None:
            raise Invalid(f"unexpected argument '{path}' (atm project logo set PATH)")
        info = get_logo(project)
        logo, chosen = info["found"], info["set"]
        if as_json:
            found = {"logo": logo["path"], "source": logo["source"]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": chosen}))
        elif logo:
            out(logo["path"] + (" (found; set one with `atm project logo set <path>`)" if logo["source"] == "found" else ""))
        elif chosen:
            out(f"meta.logo is {chosen}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm project logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if action == "unset":
        removed = unset_logo(project)
        out(to_json({"logo": None}) if as_json else "Logo unset." if removed else "No logo was set.")
        return
    if path is None:
        raise Invalid("atm project logo set needs a path")
    # The path is relative to where atm runs, like every path atm takes; atm-lib keeps it relative to the project
    relative = set_logo(project, path)
    out(to_json({"logo": relative}) if as_json else f"Logo set: {relative}")
