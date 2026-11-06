"""What `atm task state` passes to atm-lib's status functions (docs/layered-architecture.md §9, step 8)."""

# Each status function's options, as atm-lib names them (§5.1): (parameter, the command's option)
COMMAND_OPTIONS = {
    "start": (),
    "review": (("note", "note"),),
    "done": (("note", "note"), ("commit", "commit"), ("resolution", "resolution")),
    "reject": (("reason", "reason"),),
    "approve": (("note", "note"),),
    "request-changes": (("reason", "reason"),),
    "block": (("on", "on"), ("questions", "question"), ("reason", "reason")),
    "unblock": (("answers", "answer"),),
    "reopen": (("reason", "reason"),),
    "skip": (("reason", "reason"), ("resolution", "resolution"), ("duplicate_of", "duplicate_of")),
    "fail": (("reason", "reason"),),
    "release": (("note", "note"),),
}
LIST_OPTIONS = ("questions", "answers")
