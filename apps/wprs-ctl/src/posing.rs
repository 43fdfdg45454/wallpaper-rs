use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum ModelCmd {
    #[command(about = "Add a model layer from a model asset")]
    Add {
        #[arg(help = "Id of the new model layer")]
        layer: String,
        #[arg(help = "Id of the model asset it shows", long)]
        asset: String,
        #[arg(long, help = "A group to put it in")]
        into: Option<String>,
        #[arg(
            help = "Where to place it in the 3D space",
            long,
            value_name = "X,Y,Z",
            allow_hyphen_values = true
        )]
        at: Option<String>,
    },
    #[command(about = "Place a model layer or change its asset and reflection picture")]
    Set {
        #[arg(help = "The model layer")]
        layer: String,
        #[arg(
            help = "Where to place it in the 3D space",
            long,
            value_name = "X,Y,Z",
            allow_hyphen_values = true
        )]
        position: Option<String>,
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true, help = "Degrees")]
        rotation: Option<String>,
        #[arg(
            help = "Size on each axis, 1 keeps it",
            long,
            value_name = "X,Y,Z",
            allow_hyphen_values = true
        )]
        scale: Option<String>,
        #[arg(help = "Pixels of the flat box the model is drawn in", long, value_name = "W,H")]
        size: Option<String>,
        #[arg(long, help = "A picture reflected by the materials; empty removes it")]
        env: Option<String>,
        #[arg(help = "Id of another model asset to show", long)]
        asset: Option<String>,
    },
    #[command(about = "Remove a model layer")]
    Remove {
        #[arg(help = "The model layer")]
        layer: String,
    },
    #[command(about = "Describe the model of a layer or an asset")]
    Info {
        #[arg(help = "A model layer or a model asset")]
        layer: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum GroupCmd {
    #[command(about = "Make a 3D group, empty or around layers")]
    Add {
        #[arg(help = "Id of the new group")]
        id: String,
        #[arg(help = "A group to put it in", long)]
        into: Option<String>,
        #[arg(long = "layer", help = "Layers to gather in it")]
        layers: Vec<String>,
    },
    #[command(about = "Turn a group 3D, or flat with --off")]
    Space {
        #[arg(help = "The group")]
        layer: String,
        #[arg(help = "Make it flat again", long)]
        off: bool,
    },
    #[command(about = "Let a flat layer in a 3D group hide behind nearer models, or stop with --off")]
    DepthTest {
        #[arg(help = "The flat layer")]
        layer: String,
        #[arg(help = "Stop testing depth", long)]
        off: bool,
    },
    #[command(about = "Show the handles of the models, the camera frustum and the flat layers")]
    Show {
        #[arg(help = "Seconds into the scene", long, default_value_t = 0.0)]
        time: f64,
    },
}
