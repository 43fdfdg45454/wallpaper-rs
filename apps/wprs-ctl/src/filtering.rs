use anyhow::{Result, bail};
use clap::Args;
use wprs_client::Client;
use wprs_core::filter::{Filter, LOOKS, Look};

#[derive(Args, Debug)]
pub struct FilterArgs {
    #[arg(
        help = "Look: none, warm, cool, golden, vivid, punch, muted, matte, fade, mono, noir, sepia, dusk, mint, rose or night"
    )]
    pub look: Option<String>,
    #[arg(long, help = "How much of the look shows, 0 to 1 (default 1)")]
    pub strength: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "Exposure in stops, -3 to 3")]
    pub exposure: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "Contrast, -1 to 1")]
    pub contrast: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "Saturation, -1 to 1")]
    pub saturation: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "Warmth, -1 (cooler) to 1 (warmer)")]
    pub warmth: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "Hue turn in degrees, -180 to 180")]
    pub hue: Option<f32>,
    #[arg(short, long, default_value = "", help = crate::cli::OUTPUT)]
    pub output: String,
    #[arg(long, help = "List the looks")]
    pub list: bool,
}

pub fn filter(args: &FilterArgs) -> Result<Filter> {
    let look = match args.look.as_deref() {
        None => Look::None,
        Some(name) => match Look::parse(name) {
            Some(look) => look,
            None => bail!("unknown look '{name}'; see wprs-ctl filter --list"),
        },
    };
    let mut filter = Filter { look, ..Filter::default() };
    let given = [
        ("strength", args.strength),
        ("exposure", args.exposure),
        ("contrast", args.contrast),
        ("saturation", args.saturation),
        ("warmth", args.warmth),
        ("hue", args.hue),
    ];
    for (name, value) in given.into_iter().filter_map(|(n, v)| Some((n, v?))) {
        filter.set(name, value).map_err(anyhow::Error::msg)?;
    }
    Ok(filter)
}

pub async fn run(client: &Client, args: FilterArgs) -> Result<()> {
    if args.list {
        LOOKS.iter().for_each(|l| println!("{}", l.name()));
        return Ok(());
    }
    let filter = filter(&args)?;
    Ok(client.daemon.set_filter(&serde_json::to_string(&filter)?, &args.output).await?)
}
