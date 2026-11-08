"""atm's commands, one module per top-level command (`atm task`, `atm db`, `atm project`, …; doctor and service in
maintenance.py, version, help and docs in meta.py). Each defines its group and adds it to cli/app.py's `app`; the
build's MODULES order is the order they register in."""
