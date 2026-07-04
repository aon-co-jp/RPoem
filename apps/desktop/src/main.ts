/**
 * open-runo Desktop — SPA router (no external framework needed).
 * Swaps page content on sidebar-link clicks.
 */

import { healthCheck } from "./api/client";
import { renderAiRouting } from "./pages/ai-routing";
import { renderDashboard } from "./pages/dashboard";
import { renderFederation } from "./pages/federation";
import { renderSchemas } from "./pages/schemas";

// ── Page routing ─────────────────────────────────────────────────────────

type PageId = "dashboard" | "schemas" | "federation" | "ai-routing";

const content = document.getElementById("content")!;

function navigate(page: PageId): void {
  // Update active link
  document.querySelectorAll<HTMLAnchorElement>("#sidebar .nav-link").forEach((a) => {
    a.classList.toggle("active", a.dataset["page"] === page);
  });

  // Render page
  switch (page) {
    case "dashboard":
      void renderDashboard(content);
      break;
    case "schemas":
      renderSchemas(content);
      break;
    case "federation":
      renderFederation(content);
      break;
    case "ai-routing":
      renderAiRouting(content);
      break;
  }
}

// Bind sidebar links
document.querySelectorAll<HTMLAnchorElement>("#sidebar .nav-link[data-page]").forEach((a) => {
  a.addEventListener("click", (e) => {
    e.preventDefault();
    navigate(a.dataset["page"] as PageId);
  });
});

// ── Health polling ────────────────────────────────────────────────────────

async function updateHealth(): Promise<void> {
  const dot = document.getElementById("health-dot")!;
  const label = document.getElementById("health-label")!;
  try {
    const h = await healthCheck();
    dot.className = "status-dot ok";
    label.textContent = `${h.service} v${h.version}`;
  } catch {
    dot.className = "status-dot err";
    label.textContent = "unreachable";
  }
}

// Poll every 30 seconds
void updateHealth();
setInterval(() => void updateHealth(), 30_000);

// ── Initial render ────────────────────────────────────────────────────────

navigate("dashboard");
