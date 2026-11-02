"""api: the REST API `atm serve` offers the dashboard (docs/multi-project-api.md). It maps HTTP requests to atm-lib calls
and results to JSON, with the HTTP concerns (the Host check, ETags, sizes); it holds no logic and never touches
storage (docs/layered-architecture.md §1, §6)."""
