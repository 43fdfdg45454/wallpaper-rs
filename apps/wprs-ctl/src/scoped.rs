use anyhow::Result;

#[derive(clap::Args, Debug)]
pub struct SceneArgs {
    #[arg(
        long,
        global = true,
        value_name = "TEMPLATE",
        help = "Edit and show the layers of this script template instead of the layers of the scene; its own layer ids name them"
    )]
    inside: Option<String>,
    #[command(subcommand)]
    command: crate::bench::SceneCommand,
}

pub fn run(args: SceneArgs) -> Result<()> {
    wprs_editor::scope::pick(args.inside);
    crate::bench::run(args.command)
}
