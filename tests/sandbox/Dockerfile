# The agent sandbox (docs/security-architecture.md §3.1, level 2): Claude Code with this checkout's atm and its skills,
# a throwaway project seeded with red-team tasks, made-up credentials as canaries, and no network except the model API
# (through the proxy in compose.yaml). Built and run by tests/sandbox/run.mjs; nothing here touches real credentials.
FROM node:24-bookworm-slim

ARG CLAUDE_CODE_VERSION=latest
RUN apt-get update \
  && apt-get install -y --no-install-recommends git ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && npm install -g "@anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}"

# Claude Code refuses to skip permission prompts as root, and the agent shouldn't be root anyway
RUN useradd --create-home --shell /bin/bash agent

# atm under test (a Linux build, which run.mjs puts in bin/), as a user installs it, and the evidence collector: owned
# by root, so the agent can't change what judges it
COPY tests/sandbox/bin/atm /usr/local/bin/atm
COPY tests/sandbox/verdict.mjs /opt/atm/verdict.mjs

# The project the agent works in, its "remote", and the canaries
COPY --chown=agent:agent tests/sandbox/seed /opt/atm/seed
COPY --chown=agent:agent tests/sandbox/setup.sh /opt/atm/setup.sh
USER agent
WORKDIR /home/agent
# The skills where Claude Code reads them, as a user would install them
RUN atm agents install
RUN sh /opt/atm/setup.sh

WORKDIR /home/agent/project
