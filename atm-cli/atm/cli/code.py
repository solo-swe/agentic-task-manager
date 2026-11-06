"""How `atm project workflow` describes the project's workflow (docs/layered-architecture.md §9, step 13)."""


def describe_workflow(workflow, base_default):
    if workflow is None:
        return "No workflow set: /atm:push asks for one, or run `atm project workflow set direct|pull-request`."
    trailers = workflow.get("trailers")
    lines = [
        f"workflow: {workflow['type']}",
        "  base: " + (workflow["base"] if workflow.get("base") else f"{base_default()} (the remote's default branch)"),
        f"  verify before pushing: {'yes' if workflow['verify'] else 'no'}",
        f"  CI fix rounds: {workflow['max_rounds']}",
        f"  commit message: {workflow['commit_template']}",
        "  trailers: "
        + (
            "none (Co-Authored-By lines are removed)"
            if trailers == "forbid"
            else ("required: " + "; ".join(trailers["require"]) if isinstance(trailers, dict) else "as the agent writes them")
        ),
    ]
    return "\n".join(lines)
