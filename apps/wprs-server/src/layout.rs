use crate::App;
use crate::auth::{Auth, Role};
use maud::{DOCTYPE, Markup, html};

pub const STYLE: &str = "
:root{color-scheme:light dark;--fg:#1b1b1f;--bg:#fafafa;--muted:#55555f;--line:#c8c8d0;
--accent:#1d4ed8;--card:#fff}
@media (prefers-color-scheme:dark){:root{--fg:#ececf1;--bg:#141418;--muted:#b4b4c0;--line:#3a3a44;
--accent:#93b4ff;--card:#1e1e24}}
*{box-sizing:border-box}body{margin:0;font:16px/1.5 system-ui,sans-serif;color:var(--fg);background:var(--bg)}
a{color:var(--accent)}
a:focus,button:focus,input:focus,select:focus,textarea:focus{outline:3px solid var(--accent);
outline-offset:2px}
.skip{position:absolute;left:-999px}
.skip:focus{left:8px;top:8px;background:var(--card);padding:8px}
header,footer{border-bottom:1px solid var(--line);padding:8px 16px}
footer{border-top:1px solid var(--line);border-bottom:0;margin-top:32px}
nav{display:flex;flex-wrap:wrap;gap:16px;align-items:center}main{max-width:1100px;margin:0 auto;padding:16px}
form{display:flex;flex-wrap:wrap;gap:8px;align-items:end;margin:8px 0}
form.stack{flex-direction:column;align-items:stretch;max-width:560px}
label{display:flex;flex-direction:column;gap:4px;font-weight:600}
input,select,textarea,button{font:inherit;padding:6px 10px;border:1px solid var(--line);border-radius:6px;
background:var(--card);color:var(--fg)}button{cursor:pointer;font-weight:600}
ul.grid{list-style:none;padding:0;display:grid;gap:16px;
grid-template-columns:repeat(auto-fill,minmax(220px,1fr))}
ul.grid li,section.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:8px}
img{max-width:100%;height:auto;border-radius:6px}table{border-collapse:collapse;width:100%}
th,td{text-align:left;border-bottom:1px solid var(--line);padding:6px;vertical-align:top}
.warning{border-left:4px solid #c2410c;padding:4px 12px}
.veil{position:relative;display:block;overflow:hidden;border-radius:6px;cursor:pointer}
.veil img{filter:blur(28px);transform:scale(1.1)}.veil input{position:absolute;opacity:0}
.veil span{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;
padding:8px;font-weight:600;color:#fff;background:rgb(0 0 0 / .35)}
.veil input:checked~img{filter:none;transform:none}.veil input:checked~span{display:none}
.veil input:focus-visible~span{outline:3px solid var(--accent);outline-offset:-3px}
.muted,small{color:var(--muted)}dl{display:grid;grid-template-columns:max-content 1fr;gap:4px 16px}
dd{margin:0;overflow-wrap:anywhere}
";

pub fn page(app: &App, auth: Option<&Auth>, title: &str, body: Markup) -> Markup {
    let account = html! {
        @if let Some(a) = auth {
            a href="/account" { "Account (" (a.handle) ")" }
            @if a.role >= Role::Moderator { a href="/mod" { "Moderation" } }
            @if a.role >= Role::Admin { a href="/admin" { "Administration" } }
            form method="post" action="/logout" { button { "Sign out" } }
        } @else {
            a href="/login" { "Sign in" }
        }
    };
    layout(app, Some(account), title, body)
}

pub fn layout(app: &App, account: Option<Markup>, title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (app.cfg.name) }
                link rel="stylesheet" href="/static/style.css";
            }
            body {
                a.skip href="#main" { "Skip to content" }
                header {
                    nav aria-label="Site" {
                        a href="/" { strong { (app.cfg.name) } }
                        a href="/" { "Catalog" }
                        @if let Some(a) = account { (a) }
                    }
                }
                main id="main" { h1 { (title) } (body) }
                footer {
                    nav aria-label="Legal" {
                        a href="/legal" { "Legal notice" }
                        a href="/legal/terms" { "Terms" }
                        a href="/legal/privacy" { "Privacy" }
                        a href="/legal/content" { "Content policy" }
                        a href="/legal/notice" { "Report content" }
                    }
                }
            }
        }
    }
}

pub fn preview(src: &str, alt: &str, width: u32, mature: bool) -> Markup {
    html! {
        @if mature {
            label.veil {
                input type="checkbox" aria-label="Show this mature preview";
                img src=(src) alt=(alt) width=(width) loading="lazy";
                span { "Mature content: select to show the preview" }
            }
        } @else {
            img src=(src) alt=(alt) width=(width) loading="lazy";
        }
    }
}
