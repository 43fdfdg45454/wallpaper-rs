use crate::auth::Auth;
use crate::config::Config;
use crate::creds::{hex, random};
use crate::db::{now, one, rows};
use crate::error::{Problem, Res, require};
use crate::net::{Client, REPORT};
use crate::web::{page, text};
use crate::{Router, St};
use axum::extract::{Form, Path, State};
use axum::routing::get;
use maud::{Markup, html};
use rusqlite::params;
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/legal", get(operator))
        .route("/legal/{doc}", get(document))
        .route("/legal/notice", get(|s: St, a: Option<Auth>| form(s, a, false)).post(notice))
        .route("/legal/notice/{receipt}", get(receipt))
        .route("/legal/counter-notice", get(|s: St, a: Option<Auth>| form(s, a, true)).post(notice))
}

fn or_unset(value: &str) -> &str {
    if value.is_empty() { "not configured yet" } else { value }
}

async fn operator(State(app): St, auth: Option<Auth>) -> Markup {
    let c = &app.cfg;
    let agent = if c.dmca_agent.is_empty() { "none designated" } else { &c.dmca_agent };
    let body = html! {
        dl {
            dt { "Operator" } dd { (or_unset(&c.operator)) }
            dt { "Single point of contact for users and authorities" } dd { (or_unset(&c.contact)) }
            dt { "Jurisdiction" } dd { (or_unset(&c.jurisdiction)) }
            dt { "Designated copyright agent" } dd { (agent) }
        }
        p { "Report content with the " a href="/legal/notice" { "notice form" } ". "
            "Authors can answer a removal with a " a href="/legal/counter-notice" { "counter-notice" } "." }
        ul {
            li { a href="/legal/terms" { "Terms of use" } }
            li { a href="/legal/privacy" { "Privacy notice" } }
            li { a href="/legal/content" { "Content policy" } }
        }
    };
    page(&app, auth.as_ref(), "Legal notice", body)
}

async fn document(State(app): St, auth: Option<Auth>, Path(doc): Path<String>) -> Res<Markup> {
    let (title, body) = match doc.as_str() {
        "terms" => ("Terms of use", terms(&app.cfg)),
        "privacy" => ("Privacy notice", privacy(&app.cfg)),
        "content" => ("Content policy", content(&app.cfg)),
        _ => return Err(Problem::missing("page")),
    };
    Ok(page(&app, auth.as_ref(), title, body))
}

fn terms(c: &Config) -> Markup {
    html! {
        p { "This server is run by " (or_unset(&c.operator)) ". "
            "By creating an account you accept these terms." }
        h2 { "Your content" }
        p { "You keep your rights. You confirm that you may share what you upload under the licence you "
            "declare, and you allow the operator to store, sign and distribute it in the package "
            "repository of this server." }
        h2 { "Rules" }
        p { "Uploads follow the " a href="/legal/content" { "content policy" } ". "
            "Do not upload illegal content, malware or work you have no right to share." }
        h2 { "Moderation" }
        p { "Moderators may reject, remove or revoke content. Every restriction comes with a statement "
            "of reasons on your account page. Three strikes within twelve months suspend uploads; "
            "repeated or serious abuse can suspend the account." }
        h2 { "Notices and appeals" }
        p { "Anyone can send a " a href="/legal/notice" { "notice" } ". Authors can reply with a "
            a href="/legal/counter-notice" { "counter-notice" } " or write to " (or_unset(&c.contact)) "." }
        h2 { "No warranty" }
        p { "The service is provided as is, without any warranty of availability or fitness." }
    }
}

fn privacy(c: &Config) -> Markup {
    let contact = or_unset(&c.contact);
    html! {
        p { "Controller: " (or_unset(&c.operator)) ", contact " (contact) "." }
        h2 { "What is stored" }
        ul {
            li { "Your handle, a password hash (Argon2id) and your role." }
            li { "Your devices under generic names (Device 1, Device 2…) with their public keys, "
                 "and hashes of your tokens, never the tokens." }
            li { "Packages you upload and moderation records about them." }
            li { "Synced library, playlists and settings. With end-to-end encryption, the default, "
                 "the server only holds ciphertext it cannot read." }
        }
        h2 { "What is not" }
        p { "No tracking, analytics or third-party resources, and one strictly necessary session cookie. "
            "IP addresses and user agents are not logged unless the operator turns on abuse logging, "
            "which keeps them at most " (c.log_days) " days." }
        h2 { "How long" }
        p { "Sessions 30 days, device codes 10 minutes, device messages 7 days, deleted sync items "
            "180 days. Everything else until you delete it or your account." }
        h2 { "Your rights" }
        p { "Download or delete all your data from your " a href="/account" { "account page" } ". "
            "For anything else write to " (contact) "." }
    }
}

fn content(c: &Config) -> Markup {
    html! {
        ul {
            li { "Wallpapers only: images, videos and scenes in .wpak packages with a preview image." }
            li { "Accepted licences: " (c.licenses.join(", ")) "." }
            li { "No adult or mature content, no illegal content, no malware, no impersonation." }
            li { "Upload only your own work or work whose licence allows sharing, and credit its authors." }
            li { "Content converted from other formats or products needs proof that you may share it." }
            li { "Mark flashing or fast-changing animations in the description: "
                 "they can affect people with photosensitive epilepsy." }
        }
    }
}

async fn form(State(app): St, auth: Option<Auth>, counter: bool) -> Markup {
    let (title, action) = match counter {
        true => ("Counter-notice", "/legal/counter-notice"),
        false => ("Notice of illegal content", "/legal/notice"),
    };
    let statement = match counter {
        true => {
            "I state under penalty of perjury that the material was removed by mistake, \
                 and I accept the jurisdiction of the operator's courts."
        }
        false => {
            "I believe in good faith that the use is not authorised, the information is accurate and \
                  complete, and, under penalty of perjury, I am authorised to act for the rights holder."
        }
    };
    let body = html! {
        @if counter {
            p { "If your package was removed after a notice and you believe it was a mistake, tell us here." }
        } @else {
            p { "Use this form for copyright and other legal notices. "
                "You get a receipt number to follow the decision." }
        }
        form.stack method="post" action=(action) {
            @if counter {
                input type="hidden" name="kind" value="counter";
                label { "Receipt number of the notice, if you have it" input name="work" autocomplete="off"; }
            } @else {
                label { "Kind" select name="kind" {
                    option value="copyright" { "Copyright infringement" }
                    option value="illegal" { "Other illegal content" }
                } }
                label { "Protected work or law concerned" input name="work" required; }
            }
            label { "Exact address of the package page"
                input type="url" name="location" required
                    placeholder="https://example.org/packages/org.example.name"; }
            label { "Explanation" textarea name="explanation" rows="5" required minlength="20" {} }
            label { "Your name" input name="name" autocomplete="name" required; }
            label { "Your email" input type="email" name="email" autocomplete="email" required; }
            label { input type="checkbox" name="statements" value="true" required; (statement) }
            label { "Signature (type your full name)" input name="signature" required; }
            button { "Send" }
        }
    };
    page(&app, auth.as_ref(), title, body)
}

#[derive(Deserialize)]
struct Notice {
    kind: String,
    location: String,
    #[serde(default)]
    work: String,
    explanation: String,
    name: String,
    email: String,
    #[serde(default)]
    statements: bool,
    signature: String,
}

async fn notice(State(app): St, auth: Option<Auth>, client: Client, Form(n): Form<Notice>) -> Res<Markup> {
    app.limits.take(&REPORT, &client.key())?;
    let priority = [("illegal", 3), ("copyright", 2), ("counter", 2)].into_iter().find(|(k, _)| *k == n.kind);
    let priority = priority.ok_or_else(|| Problem::bad("kind: copyright, illegal or counter"))?.1;
    require(n.statements, "confirm the statements")?;
    let fields = [&n.location, &n.explanation, &n.name, &n.email, &n.signature];
    require(
        fields.iter().all(|f| !f.trim().is_empty() && f.len() <= 8000) && n.email.contains('@'),
        "fill in every field",
    )?;
    let package = n.location.split("/packages/").nth(1).and_then(|rest| rest.split(['/', '?', '#']).next());
    let package = package.filter(|p| !p.is_empty()).map(String::from);
    let package =
        package.ok_or_else(|| Problem::bad("location: the address of a package page on this server"))?;
    let (receipt, reason) = (hex(&random::<8>()), if n.kind == "counter" { "counter" } else { "notice" });
    let shown = receipt.clone();
    let latest = "SELECT id FROM versions WHERE package_id = ?1 \
                  ORDER BY (state = 'published') DESC, created_at DESC";
    let case = "INSERT INTO cases (version_id, reason, detail, priority, created_at) \
                VALUES (?1, ?2, ?3, ?4, ?5)";
    let notice = "INSERT INTO notices (id, case_id, kind, name, email, location, work, explanation, \
                  signature, received_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
    let stored = app.db.run(move |c| {
        let version: i64 = one(c, latest, [&package])?.ok_or_else(|| Problem::missing("package"))?;
        c.execute(
            case,
            params![version, reason, format!("{}: {}", n.kind, n.explanation.trim()), priority, now()],
        )?;
        let (id, at) = (c.last_insert_rowid(), now());
        let values =
            params![receipt, id, n.kind, n.name, n.email, n.location, n.work, n.explanation, n.signature, at];
        Ok(c.execute(notice, values)?)
    });
    stored.await?;
    let body = html! {
        p { "Thank you. Your receipt number is " strong { code { (shown) } } "." }
        p { "Follow the decision at " a href={ "/legal/notice/" (shown) } { "/legal/notice/" (shown) } "." }
    };
    Ok(page(&app, auth.as_ref(), "Notice received", body))
}

async fn receipt(State(app): St, auth: Option<Auth>, Path(id): Path<String>) -> Res<Markup> {
    let sql = "SELECT n.kind, n.received_at, c.state, c.decision FROM notices n \
               LEFT JOIN cases c ON c.id = n.case_id WHERE n.id = ?1";
    let found = app.db.run(move |c| rows(c, sql, [id])).await?;
    let row = found.first().ok_or_else(|| Problem::missing("notice"))?;
    let at = chrono::DateTime::from_timestamp_millis(row["received_at"].as_i64().unwrap_or_default())
        .unwrap_or_default();
    let body = html! {
        dl {
            dt { "Kind" } dd { (text(row, "kind")) }
            dt { "Received" } dd { (at.format("%Y-%m-%d %H:%M UTC")) }
            dt { "State" } dd { (row["state"].as_str().unwrap_or("closed")) }
            dt { "Decision" } dd { (row["decision"].as_str().unwrap_or("pending")) }
        }
    };
    Ok(page(&app, auth.as_ref(), "Notice status", body))
}
