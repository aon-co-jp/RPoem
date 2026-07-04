# AI Routing Engine

Implemented in [`crates/open-runo-ai-routing`](../crates/open-runo-ai-routing).

## Scope (Phase 4)

- `Provider`: OpenAI / Anthropic Claude / Google Gemini / DeepSeek /
  Local LLM / custom OpenAI-compatible endpoints.
- `RoutingPolicy`: `CostOptimized`, `LatencyOptimized`, `LocalFirst`,
  `PrivacyFirst`.
- `route(candidates, policy, min_context_length) -> Result<&Candidate>`:
  filters candidates by required context length, then picks the best one
  for the given policy (cheapest, fastest, or local-preferring).

## Design notes

- `PrivacyFirst` and `LocalFirst` currently share an implementation
  (prefer any local candidate, falling back to lowest latency). If these
  need to diverge — e.g. `PrivacyFirst` should never fall back to a cloud
  provider even if no local model qualifies — that's a policy change to
  make explicitly, with a test asserting the new behavior, rather than an
  implicit assumption.
- Cost/latency figures are caller-supplied (`Candidate` fields), not
  fetched live from providers. A future iteration could add a trait for
  pluggable cost/latency estimation sources.

## Not yet implemented

Fallback routing on provider failure, hardware-aware routing, and the
actual HTTP clients for each provider (see README §5) are not part of this
crate yet — it currently only implements the *decision* of which provider
to use, not the call itself.
