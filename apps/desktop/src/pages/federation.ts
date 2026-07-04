import { composeSchemas, getFederationStatus } from "../api/client";

export function renderFederation(container: HTMLElement): void {
  container.innerHTML = `
    <h4 class="mb-4"><i class="bi bi-diagram-3-fill me-2"></i>Federation Engine</h4>

    <!-- Status card -->
    <div class="card mb-4">
      <div class="card-header d-flex justify-content-between align-items-center">
        Current Composed Schema
        <button id="status-btn" class="btn btn-sm btn-outline-secondary">
          <i class="bi bi-arrow-clockwise"></i> Refresh
        </button>
      </div>
      <div class="card-body" id="status-body">
        <span class="text-secondary">Click Refresh to load.</span>
      </div>
    </div>

    <!-- Compose form -->
    <div class="card">
      <div class="card-header">Compose Schemas</div>
      <div class="card-body">
        <p class="text-secondary small">Enter services as JSON array matching the API spec.</p>
        <textarea id="compose-input" class="form-control form-control-sm font-monospace mb-2"
          rows="8">${JSON.stringify(
            {
              services: [
                { service_name: "users", types: { User: ["id", "name", "email"] } },
                { service_name: "billing", types: { Invoice: ["id", "amount"], User: ["id", "plan"] } },
              ],
            },
            null,
            2
          )}</textarea>
        <button id="compose-btn" class="btn btn-primary btn-sm">
          <i class="bi bi-play-fill me-1"></i>Compose
        </button>
        <div id="compose-result" class="mt-3"></div>
      </div>
    </div>
  `;

  // Status refresh
  async function loadStatus(): Promise<void> {
    const body = document.getElementById("status-body")!;
    try {
      const s = await getFederationStatus();
      body.innerHTML = `
        <div class="row g-2">
          <div class="col-auto">
            <span class="text-secondary">Services:</span>
            ${
              s.contributing_services.length === 0
                ? '<span class="text-secondary ms-1">none</span>'
                : s.contributing_services
                    .map((x) => `<span class="badge bg-secondary ms-1">${x}</span>`)
                    .join("")
            }
          </div>
          <div class="col-auto">
            <span class="text-secondary">Types:</span>
            <strong class="ms-1">${s.type_count}</strong>
          </div>
          <div class="col-auto">
            <span class="text-secondary">Fields:</span>
            <strong class="ms-1">${s.field_count}</strong>
          </div>
        </div>
      `;
    } catch (err) {
      body.innerHTML = `<span class="text-danger">${String(err)}</span>`;
    }
  }

  document.getElementById("status-btn")!.addEventListener("click", loadStatus);

  // Compose
  document.getElementById("compose-btn")!.addEventListener("click", async () => {
    const result = document.getElementById("compose-result")!;
    result.innerHTML = "Composing…";
    try {
      const raw = (document.getElementById("compose-input") as HTMLTextAreaElement).value;
      const req = JSON.parse(raw) as { services: unknown[] };
      const res = await composeSchemas(req as Parameters<typeof composeSchemas>[0]);

      const breaking =
        res.breaking_changes.length === 0
          ? `<span class="text-success">No breaking changes</span>`
          : `<div class="alert alert-warning py-1 px-2 mt-1">
               <strong>Breaking changes:</strong>
               <ul class="mb-0">${res.breaking_changes.map((b) => `<li>${b}</li>`).join("")}</ul>
             </div>`;

      result.innerHTML = `
        <div class="border rounded p-2">
          <div class="mb-1">
            ${res.contributing_services.map((s) => `<span class="badge bg-primary me-1">${s}</span>`).join("")}
          </div>
          ${breaking}
          <div class="code-block mt-2">${JSON.stringify(res.types, null, 2)}</div>
        </div>
      `;

      await loadStatus();
    } catch (err) {
      result.innerHTML = `<p class="text-danger">${String(err)}</p>`;
    }
  });
}
