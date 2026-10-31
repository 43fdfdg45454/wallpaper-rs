use crate::cli::{JSON, Wait};
use crate::private::Files;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Server {
    #[command(about = "Add the server this computer syncs with, by its https:// address")]
    Add {
        url: String,
        #[arg(long, help = "SHA-256 of the server's own certificate, as wprs-server fingerprint prints it")]
        fingerprint: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Link this computer to an account: prints the code to approve on the server")]
    Link {
        #[arg(
            long,
            default_value = "",
            help = "Name of this computer, seen only by your devices; the host name when empty"
        )]
        name: String,
        #[arg(long, help = "Sync without end-to-end encryption; the server can then read every record")]
        plaintext: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(
        about = "Show the device that waits for a code, then approve it after you confirm",
        long_about = "Show the device that waits for a code, then approve it after you confirm. Approving \
                      asks for the password of the account, read from standard input when it is not a \
                      terminal; an account that signs in through single sign-on approves on the /link page \
                      of the server instead."
    )]
    Approve {
        code: String,
        #[arg(long, help = "Approve without asking; only for a code shown on your own screen")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Show the server, this device, the sync state and the devices of the account")]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Sync now")]
    Sync {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Stop syncing automatically")]
    Pause,
    #[command(about = "Sync automatically again")]
    Resume,
    #[command(about = "Get the sync key from a device that holds it; with --wait prints the code to compare")]
    Join {
        #[arg(help = "Device that holds the key: its name, its label or its id")]
        device: String,
        #[arg(long, help = "The 6 digits shown by that device, to confirm a running join")]
        code: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Answer a joining device, show the code to compare and send the key once it matches")]
    Share {
        #[arg(help = "The joining device: its name, its label or its id")]
        device: String,
        #[arg(long, help = "The 6 digits shown by the joining device; asked for when missing")]
        code: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(
        about = "Replace the sync key, encrypt every record again and hand the key to the other devices"
    )]
    Rotate {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Delete the synced data on the server and upload this computer's copy under a new key")]
    Reset {
        #[arg(long, help = "Reset without asking")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Add a repository that another device synced, after checking it, or decline it")]
    Repo {
        #[arg(help = "The repository: its offer id or the first 8 or more digits of its repo id")]
        repo: String,
        #[arg(long, help = "Do not add it on this computer and stop asking")]
        decline: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Sync the secret of a provider with the other devices, end-to-end encrypted")]
    Secret {
        provider: String,
        #[arg(long, help = "Stop syncing it; the copies other devices hold stay")]
        stop: bool,
    },
    #[command(
        about = "Store the sync key on the server, locked with a passphrase, for when no device is left",
        long_about = "Store the sync key on the server, locked with a passphrase, for when no device is \
                      left. After a device is revoked the escrow is stale: set a new, different \
                      passphrase, since the revoked device may know the old one, which is refused."
    )]
    Escrow {
        #[arg(long, help = "Delete the escrow from the server")]
        remove: bool,
        #[arg(long, help = "Delete it without asking")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Get the sync key from the escrow with its passphrase, when no device can share it")]
    Restore {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(subcommand, about = "Your own wallpaper files, end-to-end encrypted on the server")]
    Files(Files),
    #[command(about = "Upload a .wpak package to the repository of the server")]
    Upload {
        file: String,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Forget the server and delete the token and sync key of this computer")]
    Remove {
        #[arg(long, help = "Also delete the local copies of the synced records")]
        wipe: bool,
    },
}
