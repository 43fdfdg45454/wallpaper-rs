use crate::cli::OUTPUT;
use anyhow::Result;
use clap::Args;
use wprs_client::Daemon1Proxy;

#[derive(Args, Debug)]
pub struct Out {
    #[arg(short, long, default_value = "", help = OUTPUT)]
    pub output: String,
}

#[derive(Args, Debug)]
pub struct Wait {
    #[arg(long, help = "Wait for the job, showing its progress, and print the finished job")]
    pub wait: bool,
    #[arg(long, help = "With --wait, print the finished job as JSON")]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct Page {
    #[arg(long, default_value_t = 0, help = "Number of entries to skip")]
    pub offset: u32,
    #[arg(long, default_value_t = 50, help = "Number of entries to print, at most 500")]
    pub limit: u32,
}

#[derive(Args, Debug)]
pub struct Placement {
    output: String,
    #[arg(
        long,
        value_name = "X,Y",
        default_value = "0,0",
        allow_hyphen_values = true,
        help = "Logical position"
    )]
    pos: String,
    #[arg(long, value_name = "WxH", help = "Logical size")]
    size: String,
    #[arg(long, default_value_t = 1.0)]
    scale: f64,
    #[arg(long, default_value_t = 0.0, help = "Physical pixels per millimetre, 0 when unknown")]
    density: f64,
    #[arg(long, default_value = "", help = "Maker, model and serial number of the screen")]
    identity: String,
}

impl Placement {
    pub async fn tell(self, daemon: &Daemon1Proxy<'_>) -> Result<()> {
        let Placement { output, pos, size, scale, density, identity } = self;
        let point =
            pos.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
        let (x, y) = point.ok_or_else(|| crate::usage(format!("{pos} is not X,Y")))?;
        let (width, height) = crate::editing::dimensions(&size)?;
        Ok(daemon.set_output_info(&output, x, y, width, height, scale, density, &identity).await?)
    }
}
