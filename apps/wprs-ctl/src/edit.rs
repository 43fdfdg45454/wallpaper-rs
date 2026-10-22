use crate::editing::{apply, change, export, mask, named, numbers, open, preview, set, upsert};
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;
use wprs_core::edit::{self, Kind, OutputCrop, Span};

const MASK: &str = "Mask name, or linear:X0,Y0,X1,Y1, radial:X,Y,RX,RY[,FEATHER] or luminance:LO,HI[,SOFT]";

#[derive(Subcommand, Debug)]
pub enum Edits {
    #[command(about = "Print the edit of an image as JSON")]
    Show { path: PathBuf },
    #[command(
        about = "Set ev, temperature, tint, contrast, saturation, vibrance or a path such as ops.ov.opacity"
    )]
    Set {
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE", required = true)]
        changes: Vec<String>,
    },
    #[command(about = "Crop, rotate, straighten or flip the image, or crop it for one output")]
    Crop {
        path: PathBuf,
        #[arg(
            value_name = "X,Y,W,H",
            help = "Rectangle in fractions of the image, none for all of it; kept when omitted"
        )]
        rect: Option<String>,
        #[arg(long, help = "Clockwise rotation: 0, 90, 180 or 270")]
        rotate: Option<u16>,
        #[arg(long, allow_hyphen_values = true, help = "Degrees between -45 and 45")]
        straighten: Option<f32>,
        #[arg(long, value_parser = ["none", "h", "v", "hv"])]
        flip: Option<String>,
        #[arg(short, long, help = "Output name or monitor identity that shows only this rectangle")]
        output: Option<String>,
    },
    #[command(about = "Set a tone curve through X,Y points between 0 and 1")]
    Curve {
        path: PathBuf,
        #[arg(value_name = "X,Y", required = true)]
        points: Vec<String>,
        #[arg(long, default_value = "rgb", value_parser = ["rgb", "r", "g", "b"])]
        channel: String,
        #[arg(long, default_value = "cu")]
        id: String,
    },
    #[command(about = "Blur the image; the radius is in pixels of a picture 1000 pixels long")]
    Blur {
        path: PathBuf,
        radius: f32,
        #[arg(long, help = MASK)]
        mask: Option<String>,
        #[arg(long, default_value = "bl")]
        id: String,
    },
    #[command(about = "Darken (negative amount) or lighten the edges")]
    Vignette {
        path: PathBuf,
        #[arg(allow_hyphen_values = true)]
        amount: f32,
        #[arg(long, default_value_t = 0.5)]
        midpoint: f32,
        #[arg(long, default_value_t = 0.0)]
        roundness: f32,
        #[arg(long, default_value_t = 0.5)]
        feather: f32,
        #[arg(long, default_value = "vi")]
        id: String,
    },
    #[command(about = "Lay a color over the image")]
    Overlay {
        path: PathBuf,
        #[arg(help = "Color as #rrggbb")]
        color: String,
        #[arg(long, default_value = "normal", value_parser = ["normal", "multiply", "screen", "overlay", "soft-light", "color"])]
        blend: String,
        #[arg(long, default_value_t = 1.0)]
        opacity: f32,
        #[arg(long, help = MASK)]
        mask: Option<String>,
        #[arg(long, default_value = "ov")]
        id: String,
    },
    #[command(about = "Add or replace a named mask, or remove it with none")]
    Mask {
        path: PathBuf,
        name: String,
        #[arg(help = MASK)]
        spec: String,
        #[arg(long)]
        invert: bool,
    },
    #[command(about = "Put operations first, in this order")]
    Order {
        path: PathBuf,
        #[arg(value_name = "ID", required = true)]
        ids: Vec<String>,
    },
    #[command(about = "Spread the image over the screens with the span fit")]
    Span {
        path: PathBuf,
        #[arg(long, help = "1 or more")]
        zoom: Option<f32>,
        #[arg(long, value_name = "X,Y", help = "Center of the shown part, in fractions of the image")]
        center: Option<String>,
    },
    #[command(about = "Remove operations, or the whole edit without ids")]
    Reset {
        path: PathBuf,
        #[arg(value_name = "ID")]
        ids: Vec<String>,
    },
    #[command(about = "Replace the edit with a JSON file, or - for standard input")]
    Apply { path: PathBuf, file: String },
    #[command(about = "Show changes on the desktop for 60 s without saving them")]
    Preview {
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE")]
        changes: Vec<String>,
        #[arg(long, help = "End the preview")]
        stop: bool,
    },
    #[command(about = "Render the edited image to a PNG or JPEG file; needs no daemon")]
    Export {
        path: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        #[arg(
            long,
            value_name = "WxH",
            help = "Size of the file; the size of the edited image when omitted"
        )]
        size: Option<String>,
    },
}

pub fn run(command: Edits) -> Result<()> {
    match command {
        Edits::Show { path } => print!("{}", open(&path)?.to_json()),
        Edits::Set { path, changes } => change(&path, |e| set(e, &changes))?,
        Edits::Crop { path, rect, rotate, straighten, flip, output } => change(&path, |e| {
            let none = rect.as_deref() == Some("none");
            let rect = rect.as_deref().filter(|_| !none).map(numbers::<4>).transpose()?;
            if let Some(output) = output {
                let crop =
                    OutputCrop { connector: output.clone(), crop: rect.unwrap_or([0.0, 0.0, 1.0, 1.0]) };
                drop(if none { e.outputs.remove(&output) } else { e.outputs.insert(output, crop) });
                return Ok(());
            }
            let g = &mut e.geometry;
            g.crop = if none { None } else { rect.or(g.crop) };
            g.rotate = rotate.unwrap_or(g.rotate);
            g.straighten = straighten.unwrap_or(g.straighten);
            g.flip = flip.map_or(g.flip, |f| [f.contains('h'), f.contains('v')]);
            Ok(())
        })?,
        Edits::Curve { path, points, channel, id } => change(&path, |e| {
            let points = points.iter().map(|p| numbers::<2>(p)).collect::<Result<Vec<_>>>()?;
            if !e.ops.iter().any(|op| op.id == id) {
                upsert(e, &id, Kind::Curves { rgb: vec![], r: vec![], g: vec![], b: vec![] }, None, None)?;
            }
            e.set(&format!("ops.{id}.{channel}"), &serde_json::to_string(&points)?)
                .map_err(anyhow::Error::msg)
        })?,
        Edits::Blur { path, radius, mask: spec, id } => change(&path, |e| {
            let mask = spec.map(|spec| mask(e, &id, &spec)).transpose()?;
            upsert(e, &id, Kind::Blur { radius }, mask, None)
        })?,
        Edits::Vignette { path, amount, midpoint, roundness, feather, id } => change(&path, |e| {
            upsert(e, &id, Kind::Vignette { amount, midpoint, roundness, feather }, None, None)
        })?,
        Edits::Overlay { path, color, blend, opacity, mask: spec, id } => change(&path, |e| {
            let blend = serde_json::from_value(blend.replace('-', "_").into())?;
            let mask = spec.map(|spec| mask(e, &id, &spec)).transpose()?;
            upsert(e, &id, Kind::Overlay { color, blend }, mask, Some(opacity))
        })?,
        Edits::Mask { path, name, spec, invert } => change(&path, |e| match spec.as_str() {
            "none" => {
                e.ops.iter_mut().filter(|op| op.mask.as_ref() == Some(&name)).for_each(|op| op.mask = None);
                e.masks.remove(&name).map(drop).with_context(|| format!("no mask is named {name}"))
            }
            spec => named(e, &name, spec, invert),
        })?,
        Edits::Order { path, ids } => change(&path, |e| {
            if let Some(id) = ids.iter().find(|id| !e.ops.iter().any(|op| op.id == **id)) {
                anyhow::bail!("no operation is named {id}");
            }
            e.ops.sort_by_key(|op| ids.iter().position(|id| *id == op.id).unwrap_or(usize::MAX));
            Ok(())
        })?,
        Edits::Span { path, zoom, center } => change(&path, |e| {
            let span = e.span.get_or_insert_with(Span::default);
            span.zoom = zoom.unwrap_or(span.zoom);
            span.center = center.as_deref().map(numbers::<2>).transpose()?.unwrap_or(span.center);
            Ok(())
        })?,
        Edits::Reset { path, ids } if ids.is_empty() => {
            edit::remove(&path, &edit::dir()).map_err(anyhow::Error::msg)?;
        }
        Edits::Reset { path, ids } => change(&path, |e| {
            let before = e.ops.len();
            e.ops.retain(|op| !ids.contains(&op.id));
            let gone = before - e.ops.len();
            anyhow::ensure!(gone == ids.len(), "some of {} are not operations", ids.join(" "));
            Ok(())
        })?,
        Edits::Apply { path, file } => apply(&path, &file)?,
        Edits::Preview { path, changes, stop } => preview(&path, &changes, stop)?,
        Edits::Export { path, out, quality, size } => export(&path, &out, quality, size.as_deref())?,
    }
    Ok(())
}
