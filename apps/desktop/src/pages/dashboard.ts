import { getFederationStatus, healthCheck } from "../api/client";

export async function renderDashboard(container: HTMLElement): Promise<void> {
  container.innerHTML = `
    <h4 class="mb-4"><i class="bi bi-grid-1x2-fill me-2"></i>Dashboard</h4>
    <div class="row g-3" id="dashboard-cards">
      <div class="col-12 text-secondary">Loading…</div>
    </div>
  `;

  try {
    const [health, fed] = await Promise.all([
      healthCheck(),
      getFederationStatus(),
    ]);

    const cards = document.getElementById("dashboard-cards")!;
    cards.innerHTML = `
      <div class="col-md-4">
        <div class="card h-100">
          <div class="card-body">
            <h6 class="card-subtitle text-secondary mb-1">
              <i class="bi bi-heart-pulse me-1"></i>Gateway Status
            </h6>
            <p class="card-title fs-4 mb-0 text-success fw-bold">${health.status.toUpperCase()}</p>
            <small class="text-secondary">${health.service} v${health.version}</small>
          </div>
        </div>
      </div>
      <div class="col-md-4">
        <div class="card h-100">
          <div class="card-body">
            <h6 class="card-subtitle text-secondary mb-1">
              <i class="bi bi-diagram-3-fill me-1"></i>Federation
            </h6>
            <p class="card-title fs-4 mb-0 fw-bold">${fed.contributing_services.length} services</p>
            <small class="text-secondary">${fed.type_count} types · ${fed.field_count} fields</small>
          </div>
        </div>
      </div>
      <div class="col-md-4">
        <div class="card h-100">
          <div class="card-body">
            <h6 class="card-subtitle text-secondary mb-1">
              <i class="bi bi-layers me-1"></i>Services
            </h6>
            ${
              fed.contributing_services.length === 0
                ? '<p class="text-secondary mb-0">No services registered</p>'
                : fed.contributing_services
                    .map(
                      (s) =>
                        `<span class="badge bg-secondary me-1">${s}</span>`
                    )
                    .join("")
            }
          </div>
        </div>
      </div>
    `;
  } catch (err) {
    container.innerHTML = `
      <div class="alert alert-danger">
        <i class="bi bi-exclamation-triangle me-2"></i>
        Failed to load dashboard: ${String(err)}
      </div>
    `;
  }
}
