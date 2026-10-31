use crate::config::Config;
use crate::legal::or_unset;
use maud::{Markup, html};

pub fn terms(c: &Config) -> Markup {
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

pub fn privacy(c: &Config) -> Markup {
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
            "180 days, the name, email and signature of a legal notice " (c.notice_days) " days, a "
            "package withdrawn by the moderators " (c.takedown_days) " days, as evidence and for a "
            "counter-notice. "
            "Everything else until you delete it or your account." }
        h2 { "Your rights" }
        p { "Download or delete all your data from your " a href="/account" { "account page" } ". "
            "For anything else write to " (contact) "." }
    }
}

pub fn content(c: &Config) -> Markup {
    html! {
        ul {
            li { "Wallpapers only: images, videos and scenes in .wpak packages with a preview image." }
            li { "Accepted licences: " (c.licenses.join(", ")) "." }
            li { "Mature content is accepted only when the package rates it mature in its manifest \
                (\"content\": {\"rating\": \"mature\"} with its reasons): it carries that label in the catalog, \
                in search and in the API, and clients and this site blur its previews until the viewer \
                chooses to show them. Mature content without that rating is removed." }
            li { "No illegal content, no sexual content involving minors, no malware, no impersonation." }
            li { "Upload only your own work or work whose licence allows sharing, and credit its authors." }
            li { "Content converted from other formats or products needs proof that you may share it." }
            li { "Packages that flash or show fast patterns must set \"photosensitive\": true in their \
                wallpaper-rs.wpak/2 manifest: they can trigger seizures in people with photosensitive \
                epilepsy, and clients warn before showing them." }
        }
    }
}
