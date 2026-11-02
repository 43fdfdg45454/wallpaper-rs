use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum ModelCmd {
    #[command(about = "Add a model layer from a model asset")]
    Add {
        layer: String,
        #[arg(long)]
        asset: String,
        #[arg(long, help = "A group to put it in")]
        into: Option<String>,
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        at: Option<String>,
    },
    #[command(about = "Place a model layer or change its asset and reflection picture")]
    Set {
        layer: String,
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        position: Option<String>,
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true, help = "Degrees")]
        rotation: Option<String>,
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        scale: Option<String>,
        #[arg(long, value_name = "W,H")]
        size: Option<String>,
        #[arg(long, help = "A picture reflected by the materials; empty removes it")]
        env: Option<String>,
        #[arg(long)]
        asset: Option<String>,
    },
    #[command(about = "Remove a model layer")]
    Remove { layer: String },
    #[command(about = "Describe the model of a layer or an asset")]
    Info { layer: String },
}

#[derive(Subcommand, Debug)]
pub enum GroupCmd {
    #[command(about = "Make a 3D group, empty or around layers")]
    Add {
        id: String,
        #[arg(long)]
        into: Option<String>,
        #[arg(long = "layer", help = "Layers to gather in it")]
        layers: Vec<String>,
    },
    #[command(about = "Turn a group 3D, or flat with --off")]
    Space {
        layer: String,
        #[arg(long)]
        off: bool,
    },
    #[command(about = "Let a flat layer in a 3D group hide behind nearer models, or stop with --off")]
    DepthTest {
        layer: String,
        #[arg(long)]
        off: bool,
    },
    #[command(about = "Show the handles of the models, the camera frustum and the flat layers")]
    Show {
        #[arg(long, default_value_t = 0.0)]
        time: f64,
    },
}
