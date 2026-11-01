// 検査中のページの window.__openRunoRpcLog(JSON文字列)を1秒ごとに読んで表で出す。
const rows = document.getElementById("rows");
const info = document.getElementById("info");
let last = "";
let cleared = 0;

function render(entries) {
  rows.textContent = "";
  for (const e of entries.slice(cleared)) {
    const tr = document.createElement("tr");
    tr.className = "row";
    const cells = [new Date(e.at).toLocaleTimeString(), e.method, e.wire, e.http, e.grpcName, Math.round(e.ms)];
    cells.forEach((c, i) => {
      const td = document.createElement("td");
      td.textContent = c;
      if ((i === 3 && e.http >= 400) || (i === 4 && e.grpc !== 0)) td.className = "err";
      tr.appendChild(td);
    });
    const detail = document.createElement("tr");
    detail.hidden = true;
    const dtd = document.createElement("td");
    dtd.colSpan = 6;
    const pre = document.createElement("pre");
    pre.textContent = "request:  " + e.request + "\nresponse: " + e.response;
    dtd.appendChild(pre);
    detail.appendChild(dtd);
    tr.addEventListener("click", () => { detail.hidden = !detail.hidden; });
    rows.append(tr, detail);
  }
  info.textContent = entries.length - cleared + " 件";
}

function poll() {
  chrome.devtools.inspectedWindow.eval("window.__openRunoRpcLog || ''", (text, err) => {
    if (err || typeof text !== "string" || text === last) return;
    last = text;
    try { render(text ? JSON.parse(text) : []); } catch (_) {}
  });
}

document.getElementById("clear").addEventListener("click", () => {
  try { cleared = JSON.parse(last || "[]").length; } catch (_) { cleared = 0; }
  render(JSON.parse(last || "[]"));
});
setInterval(poll, 1000);
poll();
