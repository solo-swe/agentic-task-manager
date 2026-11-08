"""`atm version`, `atm help` and `atm docs`: the commands about atm itself."""

from typing import Optional

import typer

from ...common.core import AtmError, out, plugin_version, to_json
from ..agent_docs import AGENT_DOCS
from ..app import META, app


@app.command("version", rich_help_panel=META)
def version(ctx: typer.Context) -> None:
    """Print the plugin version."""
    out(to_json({"version": plugin_version()}) if ctx.obj.get("json_out", False) else plugin_version())


@app.command("help", rich_help_panel=META)
def help_(ctx: typer.Context) -> None:
    """Show the list of commands."""
    usage = ctx.parent.get_help()
    out(to_json({"usage": usage}) if ctx.obj.get("json_out", False) else usage)


@app.command("docs", rich_help_panel=META)
def docs(
    ctx: typer.Context,
    topic: Optional[str] = typer.Argument(None, help="The topic to print; without one, the list of topics"),
) -> None:
    """Reference for agents, for this version of atm: the rules, the commands, the states, claims, the task fields."""
    as_json = ctx.obj.get("json_out", False)
    if topic is None:
        if as_json:
            out(to_json([{"topic": name, "summary": summary} for name, (summary, _) in AGENT_DOCS.items()]))
        else:
            width = max(len(name) for name in AGENT_DOCS)
            out("\n".join(f"{name.ljust(width)}  {summary}" for name, (summary, _) in AGENT_DOCS.items()))
        return
    if topic not in AGENT_DOCS:
        raise AtmError(f"no topic '{topic}' (atm docs lists them: {', '.join(AGENT_DOCS)})", 2)
    summary, text = AGENT_DOCS[topic]
    out(to_json({"topic": topic, "summary": summary, "text": text}) if as_json else text.rstrip("\n"))
