import { aiRoute } from "../api/client";
import type { AiCandidate, AiRouteRequest, RoutingPolicy } from "../api/types";

const DEFAULT_CANDIDATES: AiCandidate[] = [
  {
    provider: "local_llm",
    estimated_cost_usd_per_1k_tokens: 0.0,
    estimated_latency_ms: 900,
    is_local: true,
    context_length: 8_000,
  },
  {
    provider: "anthropic_claude",
    estimated_cost_usd_per_1k_tokens: 3.0,
    estimated_latency_ms: 400,
    is_local: false,
    context_length: 200_000,
  },
  {
    provider: "openai",
    estimated_cost_usd_per_1k_tokens: 2.0,
    estimated_latency_ms: 500,
    is_local: false,
    context_length: 128_000,
  },
  {
    provider: "deepseek",
    estimated_cost_usd_per_1k_tokens: 0.14,
    estimated_latency_ms: 600,
    is_local: false,
    context_length: 65_000,
  },
];

export function renderAiRouting(container: HTMLElement): void {
  container.innerHTML = `
    <h4 class="mb-4"><i class="bi bi-robot me-2"></i>AI Routing Engine</h4>

    <div class="card">
      <div class="card-header">Select Best Provider</div>
      <div class="card-body">
        <div class="row g-3 mb-3">
          <div class="col-md-4">
            <label class="form-label">Routing Policy</label>
            <select id="ai-policy" class="form-select form-select-sm">
              <option value="cost">Cost Optimized</option>
              <option value="latency">Latency Optimized</option>
              <option value="local">Local First</option>
              <option value="privacy">Privacy First</option>
            </select>
          </div>
          <div class="col-md-4">
            <label class="form-label">Min Context Length (tokens)</label>
            <input id="ai-ctx" type="number" class="form-control form-control-sm"
              value="4000" min="0" />
          </div>
        </div>

        <label class="form-label">Candidates (JSON)</label>
        <textarea id="ai-candidates" class="form-control form-control-sm font-monospace mb-2"
          rows="14">${JSON.stringify(DEFAULT_CANDIDATES, null, 2)}</textarea>

        <button id="ai-route-btn" class="btn btn-primary btn-sm">
          <i class="bi bi-lightning-charge-fill me-1"></i>Find Best Provider
        </button>
        <div id="ai-result" class="mt-3"></div>
      </div>
    </div>
  `;

  document.getElementById("ai-route-btn")!.addEventListener("click", async () => {
    const result = document.getElementById("ai-result")!;
    result.innerHTML = "Routing…";
    try {
      const policy = (document.getElementById("ai-policy") as HTMLSelectElement)
        .value as RoutingPolicy;
      const minCtx = parseInt(
        (document.getElementById("ai-ctx") as HTMLInputElement).value,
        10
      );
      const candidates = JSON.parse(
        (document.getElementById("ai-candidates") as HTMLTextAreaElement).value
      ) as AiCandidate[];

      const req: AiRouteRequest = {
        policy,
        min_context_length: minCtx,
        candidates,
      };

      const res = await aiRoute(req);

      result.innerHTML = `
        <div class="card border-success">
          <div class="card-body">
            <h6 class="card-title text-success mb-3">
              <i class="bi bi-check-circle-fill me-1"></i>Selected Provider
            </h6>
            <div class="row g-2">
              <div class="col-auto">
                <span class="badge bg-primary fs-6">${res.selected_provider}</span>
                ${res.is_local ? '<span class="badge bg-success ms-1">local</span>' : '<span class="badge bg-info ms-1">cloud</span>'}
              </div>
            </div>
            <table class="table table-sm mt-3 mb-0">
              <tr>
                <td class="text-secondary">Estimated cost</td>
                <td><strong>$${res.estimated_cost_usd_per_1k_tokens.toFixed(4)}</strong> / 1k tokens</td>
              </tr>
              <tr>
                <td class="text-secondary">Estimated latency</td>
                <td><strong>${res.estimated_latency_ms} ms</strong></td>
              </tr>
            </table>
          </div>
        </div>
      `;
    } catch (err) {
      result.innerHTML = `<div class="alert alert-danger">${String(err)}</div>`;
    }
  });
}
