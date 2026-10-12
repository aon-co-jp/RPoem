//! open-runo-crossui — 一度書いたフロントエンドを、Windows / macOS / Linux / Android / Web 向けに自動で生成する。
//!
//! 入力: [`AppSpec`](アプリ名・エンジンの場所・`open-runo-view` の VNode ツリー・エンジンとの橋渡し [`Bridge`])。
//! 出力: [`generate`] が `out/<target>/` に、共有のページ(SSR された HTML + Rust から生成した橋渡し JS)と、
//! OS ごとの起動・配布の記述(Windows の `launch.cmd`、macOS の `launch.command`、Linux の `.desktop`、
//! Android の WebView 用アセットとマニフェスト)を書く。ページは全 OS で同一なので、本体(Windows 版)を直すと全 OS に反映される。

use open_runo_view::ssr::{render_page, SsrPage};
use open_runo_view::{h, VNode};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Windows,
    MacOs,
    Linux,
    Android,
    Web,
}

pub const ALL_TARGETS: &[Target] = &[Target::Windows, Target::MacOs, Target::Linux, Target::Android, Target::Web];

impl Target {
    pub fn dir(self) -> &'static str {
        match self {
            Target::Windows => "windows",
            Target::MacOs => "macos",
            Target::Linux => "linux",
            Target::Android => "android",
            Target::Web => "web",
        }
    }
}

/// エンジン(常駐プロセス)との橋渡し。キー入力を JSON でエンジンに送り、返ってきた確定文字・組み立て中の文字・候補を画面に反映する。
#[derive(Debug, Clone)]
pub struct Bridge {
    /// キー入力の送り先(POST)。
    pub key_path: String,
    /// 起動確認(GET)。
    pub health_path: String,
    /// 入力欄・組み立て中・候補欄・状態表示の要素 id。
    pub input_id: String,
    pub preedit_id: String,
    pub cands_id: String,
    pub status_id: String,
}

impl Default for Bridge {
    fn default() -> Self {
        Self { key_path: "/key".into(), health_path: "/health".into(), input_id: "t".into(), preedit_id: "pre".into(), cands_id: "c".into(), status_id: "st".into() }
    }
}

pub struct AppSpec {
    pub id: String,
    pub name: String,
    pub version: String,
    /// エンジンの URL(例 `http://127.0.0.1:4660`)。
    pub engine_url: String,
    pub bridge: Bridge,
    pub root: VNode,
    pub css: String,
}

fn js_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '<' => o.push_str("\\u003c"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// 橋渡しの JS(Rust のテンプレートから生成する)。
pub fn bridge_js(b: &Bridge, engine_base: &str) -> String {
    format!(
        r##"(()=>{{
const BASE={base},KEY={key},HEALTH={health};
const el=i=>document.getElementById(i),t=el({input}),pre=el({pre}),c=el({cands}),st=el({status});
const sid="web-"+Math.random().toString(36).slice(2);
const post=(p,b)=>fetch(BASE+p,{{method:"POST",body:JSON.stringify(b)}}).then(r=>r.json());
let composing=false;
function keyOf(e){{
  if(e.ctrlKey||e.metaKey||e.altKey)return null;
  const m={{" ":"Space","Enter":"Enter","Escape":"Esc","Backspace":"Backspace","ArrowUp":"Up","ArrowDown":"Down","ArrowLeft":"Left","ArrowRight":"Right","Tab":"Tab","Convert":"Convert","NonConvert":"NonConvert"}};
  if(m[e.key]){{ if(!composing&&["Up","Down","Left","Right","Tab","Backspace","Enter","Esc"].includes(m[e.key]))return null; return m[e.key]; }}
  if(/^F([6-9]|10)$/.test(e.key)&&composing)return {{Fn:+e.key.slice(1)}};
  if(e.key.length===1)return {{Char:e.key}};
  return null;
}}
t.addEventListener("keydown",async e=>{{
  const k=keyOf(e); if(!k)return;
  const before=t.value.slice(0,t.selectionStart).slice(-40);
  const r=await post(KEY,{{sid,key:k,before}});
  if(!r.eaten&&!r.commit)return;
  e.preventDefault();
  if(r.commit){{t.setRangeText(r.commit,t.selectionStart,t.selectionEnd,"end");}}
  composing=!!r.composing;
  pre.textContent=r.preedit||"";
  c.innerHTML=(r.cands||[]).slice(0,10).map((x,i)=>'<div class="'+(r.sel===i?"sel":"")+'">'+((i+1)%10)+". "+x.replace(/&/g,"&amp;").replace(/</g,"&lt;")+"</div>").join("");
  st.textContent=(r.lang||"")+(r.on===false?" (OFF)":"");
}});
const help=document.getElementById("help");
fetch(BASE+HEALTH).then(r=>r.text()).then(x=>{{st.textContent=x;if(help)help.hidden=true;}}).catch(e=>{{st.textContent="engine offline";if(help){{help.hidden=false;const d=document.getElementById("helpwhy");if(d)d.textContent=" ["+(e&&e.name||"error")+": "+(e&&e.message||"")+" / "+location.origin+"]";}}}});
t.addEventListener("focus",()=>{{if(help&&st.textContent==="engine offline"){{fetch(BASE+HEALTH).then(r=>r.text()).then(x=>{{st.textContent=x;help.hidden=true;}}).catch(()=>{{}});}}}});
}})();"##,
        base = js_str(engine_base),
        key = js_str(&b.key_path),
        health = js_str(&b.health_path),
        input = js_str(&b.input_id),
        pre = js_str(&b.preedit_id),
        cands = js_str(&b.cands_id),
        status = js_str(&b.status_id),
    )
}

/// 全 OS 共通のページ(SSR された HTML)。`engine_base` が空なら同一オリジン(エンジン自身が配る場合)。
pub fn page_html(app: &AppSpec, engine_base: &str) -> String {
    let head = format!("<style>{}</style>", app.css);
    let scripts = format!("<script>{}</script>", bridge_js(&app.bridge, engine_base));
    let mut p = SsrPage::default_for(&app.root);
    p.title = &app.name;
    p.head_extra = &head;
    p.scripts = &scripts;
    render_page(&p)
}

fn write(out: &Path, rel: &str, body: &str, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let p = out.join(rel);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&p, body)?;
    files.push(p);
    Ok(())
}

fn slug(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect()
}

/// 指定した OS 向けの成果物を `out/<target>/` に書く。書いたファイルを返す。
pub fn generate(app: &AppSpec, targets: &[Target], out: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = vec![];
    let html = page_html(app, &app.engine_url);
    let id = slug(&app.id);
    for t in targets {
        let d = t.dir();
        write(out, &format!("{d}/index.html"), &html, &mut files)?;
        let (name, ver, url) = (&app.name, &app.version, &app.engine_url);
        match t {
            Target::Web => {}
            Target::Windows => write(
                out,
                "windows/launch.cmd",
                &format!("@echo off\r\nrem {name} {ver}\r\nstart \"\" /b \"%~dp0..\\..\\aruaru-ime-daemon.exe\"\r\nstart \"\" \"{url}/ui\"\r\n"),
                &mut files,
            )?,
            Target::MacOs => write(
                out,
                "macos/launch.command",
                &format!("#!/bin/sh\n# {name} {ver}\ncd \"$(dirname \"$0\")\"\n(\"../../aruaru-ime-daemon\" >/dev/null 2>&1 &)\nopen \"{url}/ui\"\n"),
                &mut files,
            )?,
            Target::Linux => {
                write(
                    out,
                    "linux/launch.sh",
                    &format!("#!/bin/sh\n# {name} {ver}\ncd \"$(dirname \"$0\")\"\n(\"../../aruaru-ime-daemon\" >/dev/null 2>&1 &)\nxdg-open \"{url}/ui\"\n"),
                    &mut files,
                )?;
                write(out, &format!("linux/{id}.desktop"), &format!("[Desktop Entry]\nType=Application\nName={name}\nExec=sh launch.sh\nTerminal=false\nCategories=Utility;\n"), &mut files)?;
            }
            Target::Android => {
                write(
                    out,
                    "android/manifest.json",
                    &format!("{{\"id\":{},\"name\":{},\"version\":{},\"entry\":\"index.html\",\"engine\":{},\"shell\":\"webview\"}}\n", js_str(&app.id), js_str(name), js_str(ver), js_str(url)),
                    &mut files,
                )?;
                write(
                    out,
                    "android/README.txt",
                    "このフォルダを Android アプリ(WebView シェル)の assets に入れる。index.html を読み込み、manifest.json の engine へ接続する。\nPut this folder in the app's assets; the WebView loads index.html and talks to the engine in manifest.json.\n",
                    &mut files,
                )?;
            }
        }
    }
    Ok(files)
}

/// 入力欄・組み立て中・候補欄を持つ、IME 向けの標準の画面。
pub fn ime_root(b: &Bridge) -> VNode {
    ime_root_with(b, DOWNLOAD_URL, "http://127.0.0.1:4660/ui")
}

/// ダウンロードの場所(リリース一覧)。
pub const DOWNLOAD_URL: &str = "https://github.com/aon-co-jp/aruaru-IME/releases";

/// 入力欄に加えて、ダウンロードのリンクと、エンジンに接続できないときの案内を持つ画面。
pub fn ime_root_with(b: &Bridge, download_url: &str, local_ui: &str) -> VNode {
    h("main")
        .child(h("h3").child(VNode::Text("aruaru-IME ".into())).child(h("small").attr("id", &b.status_id).build()).build())
        .child(h("textarea").attr("id", &b.input_id).attr("autofocus", "").attr("placeholder", "ここに入力(ローマ字)。Space で変換、Enter で確定、数字で候補選択").build())
        .child(h("div").attr("id", &b.preedit_id).build())
        .child(h("div").attr("id", &b.cands_id).build())
        .child(
            h("p")
                .child(h("a").attr("href", download_url).attr("id", "dl").child(VNode::Text("ダウンロードはこちら(インストーラー) / Download the installer".into())).build())
                .child(VNode::Text("  |  ".into()))
                .child(h("a").attr("href", local_ui).child(VNode::Text("このPCの画面で開く / Open on this PC".into())).build())
                .build(),
        )
        .child(
            h("div")
                .attr("id", "help")
                .attr("hidden", "")
                .child(VNode::Text("エンジンに接続できません。(1) aruaru-IME をインストールして起動する (2) ブラウザが「ローカルネットワークへのアクセス」の許可を求めたら許可する (3) 上の「このPCの画面で開く」から使う。 / Cannot reach the engine: install and start aruaru-IME, allow local network access when asked, or use the on-this-PC link.".into()))
                .child(h("span").attr("id", "helpwhy").build())
                .build(),
        )
        .build()
}

pub const IME_CSS: &str = ":root{--bg:#111;--fg:#fff;--acc:#4a9eff}@media (prefers-color-scheme:light){:root{--bg:#fff;--fg:#111}}\
body{margin:0;background:var(--bg);color:var(--fg);font:18px system-ui,sans-serif;padding:16px}\
textarea{width:100%;box-sizing:border-box;min-height:40vh;background:transparent;color:inherit;border:1px solid #888;border-radius:8px;padding:12px;font:inherit}\
#pre{margin:8px 0;min-height:1.6em;color:var(--acc);text-decoration:underline}\
#help{margin:8px 0;padding:8px 12px;border:1px solid #c66;border-radius:6px}a{color:var(--acc)}#c{display:grid;grid-template-columns:1fr 1fr;gap:4px 16px}#c div{padding:6px 10px;border-radius:6px;font-size:1.3em}#c .sel{background:var(--acc);color:#fff}";

/// aruaru-IME の標準の定義。
pub fn ime_app(engine_url: &str, version: &str) -> AppSpec {
    let b = Bridge::default();
    AppSpec { id: "aruaru-ime".into(), name: "aruaru-IME".into(), version: version.into(), engine_url: engine_url.into(), root: ime_root(&b), bridge: b, css: IME_CSS.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_has_ssr_body_and_generated_bridge() {
        let h = page_html(&ime_app("http://127.0.0.1:4660", "0.1.0"), "");
        assert!(h.contains("<textarea") && h.contains("id=\"t\""), "{h}");
        assert!(h.contains("fetch(BASE+HEALTH)") && h.contains("const BASE=\"\""));
        assert!(h.contains("<title>aruaru-IME</title>"));
        assert!(h.contains("releases") && h.contains("ダウンロードはこちら") && h.contains("id=\"help\""), "ダウンロードのリンクと接続の案内");
    }

    #[test]
    fn generates_every_target_from_one_definition() {
        let d = std::env::temp_dir().join(format!("crossui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        generate(&ime_app("http://127.0.0.1:4660", "0.1.0"), ALL_TARGETS, &d).unwrap();
        for t in ALL_TARGETS {
            assert!(d.join(t.dir()).join("index.html").is_file(), "{t:?}");
        }
        assert!(d.join("windows/launch.cmd").is_file() && d.join("macos/launch.command").is_file());
        assert!(d.join("linux/aruaru-ime.desktop").is_file());
        assert!(std::fs::read_to_string(d.join("android/manifest.json")).unwrap().contains("\"shell\":\"webview\""));
        assert_eq!(std::fs::read_to_string(d.join("windows/index.html")).unwrap(), std::fs::read_to_string(d.join("android/index.html")).unwrap(), "ページは全OS共通");
    }

    #[test]
    fn js_strings_are_escaped() {
        assert_eq!(js_str("a\"b</script>"), "\"a\\\"b\\u003c/script>\"");
    }
}
