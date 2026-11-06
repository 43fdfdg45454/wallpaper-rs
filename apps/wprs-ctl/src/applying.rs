use crate::cli::{FIT, ITEM, OUTPUT};
use crate::input::one;
use anyhow::Result;
use clap::Args;
use wprs_client::Client;
use wprs_core::Fit;

#[derive(Args, Debug)]
pub struct Applying {
    #[arg(value_name = "ITEM", help = ITEM)]
    item: String,
    #[arg(short, long, default_value = "", help = OUTPUT)]
    output: String,
    #[arg(short, long, help = FIT)]
    fit: Option<Fit>,
    #[arg(long, conflicts_with = "fit", help = "Spread it over every screen, as --fit span")]
    span: bool,
    #[arg(short, long, default_value = "", help = "Preset uid or name")]
    preset: String,
}

impl Applying {
    pub async fn run(self, client: &Client) -> Result<()> {
        let Applying { item, output, fit, span, preset } = self;
        let (uid, fit) = (one(client, &item).await?, if span { Some(Fit::Span) } else { fit });
        crate::caution::apply(client, &uid, &output, fit.map_or("", Fit::as_str), &preset).await
    }
}
