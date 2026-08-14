# Local-only Open Design fork

This directory is forked from
[`nexu-io/open-design`](https://github.com/nexu-io/open-design) at the latest
stable `open-design-v0.19.2` tag (`a539ba57ce3ad4200b0a300007da82783ec66e12`).

The fork permanently disables Open Design Cloud/AMR authentication. It keeps
local coding-agent CLIs and user-supplied API providers (BYOK) as the supported
execution paths.

The daemon returns HTTP 410 for `/api/amr/*` and
`/api/integrations/vela/*`, so login cannot be restored accidentally by an old
UI, plugin, CLI, or direct API caller. The web app skips cloud identity during
onboarding and hides the hosted runtime and cloud account affordances.

When rebasing on upstream, preserve these local-only boundaries and run the
daemon route, web component, typecheck, and guard suites before updating this
file's upstream tag and commit.
