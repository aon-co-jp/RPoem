import { getSchemaHistory, registerSchema } from "../api/client";
import type { Stage } from "../api/types";

export function renderSchemas(container: HTMLElement): void {
  container.innerHTML = `
    <h4 class="mb-4"><i class="bi bi-file-code me-2"></i>Schema Registry</h4>

    <!-- Register form -->
    <div class="card mb-4">
      <div class="card-header">Register Schema</div>
      <div class="card-body">
        <form id="schema-form">
          <div class="row g-2">
            <div class="col-md-4">
              <label class="form-label">Service Name</label>
              <input id="svc-name" class="form-control form-control-sm" placeholder="users-service" required />
            </div>
            <div class="col-md-3">
              <label class="form-label">Stage</label>
              <select id="svc-stage" class="form-select form-select-sm">
                <option value="local">local</option>
                <option value="development">development</option>
                <option value="staging">staging</option>
                <option value="production">production</option>
              </select>
            </div>
          </div>
          <div class="mt-2">
            <label class="form-label">SDL</label>
            <textarea id="svc-sdl" class="form-control form-control-sm font-monospace"
              rows="4" placeholder="type User { id: ID! name: String }" required></textarea>
          </div>
          <button type="submit" class="btn btn-primary btn-sm mt-2">
            <i class="bi bi-cloud-upload me-1"></i>Register
          </button>
          <span id="schema-msg" class="ms-2 small"></span>
        </form>
      </div>
    </div>

    <!-- History lookup -->
    <div class="card">
      <div class="card-header">Schema History</div>
      <div class="card-body">
        <div class="input-group input-group-sm mb-3" style="max-width:320px;">
          <input id="hist-svc" class="form-control" placeholder="service name" />
          <button id="hist-btn" class="btn btn-outline-secondary">
            <i class="bi bi-search"></i> Fetch
          </button>
        </div>
        <div id="history-list"></div>
      </div>
    </div>
  `;

  // Register form
  document.getElementById("schema-form")!.addEventListener("submit", async (e) => {
    e.preventDefault();
    const msg = document.getElementById("schema-msg")!;
    msg.textContent = "Registering…";
    try {
      const res = await registerSchema({
        service_name: (document.getElementById("svc-name") as HTMLInputElement).value,
        sdl: (document.getElementById("svc-sdl") as HTMLTextAreaElement).value,
        stage: (document.getElementById("svc-stage") as HTMLSelectElement).value as Stage,
      });
      msg.className = "ms-2 small text-success";
      msg.textContent = `✓ Registered — id: ${res.id.slice(0, 8)}…`;
    } catch (err) {
      msg.className = "ms-2 small text-danger";
      msg.textContent = `✗ ${String(err)}`;
    }
  });

  // History fetch
  document.getElementById("hist-btn")!.addEventListener("click", async () => {
    const svc = (document.getElementById("hist-svc") as HTMLInputElement).value.trim();
    if (!svc) return;
    const list = document.getElementById("history-list")!;
    list.innerHTML = "Loading…";
    try {
      const { versions } = await getSchemaHistory(svc);
      if (versions.length === 0) {
        list.innerHTML = `<p class="text-secondary">No versions found for "${svc}".</p>`;
        return;
      }
      list.innerHTML = versions
        .slice()
        .reverse()
        .map(
          (v) => `
          <div class="border rounded p-2 mb-2">
            <div class="d-flex justify-content-between align-items-center">
              <span class="badge bg-secondary">${v.stage}</span>
              <small class="text-secondary">${new Date(v.created_at).toLocaleString()}</small>
            </div>
            <div class="code-block mt-1">${escHtml(v.sdl)}</div>
          </div>
        `
        )
        .join("");
    } catch (err) {
      list.innerHTML = `<p class="text-danger">${String(err)}</p>`;
    }
  });
}

function escHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
