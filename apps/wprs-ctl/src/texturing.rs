use crate::author::DIR;
use crate::show::bytes;
use anyhow::{Result, anyhow};
use clap::{ArgAction, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_editor::actions::{Action, AssetEdit, RgbEdit};
use wprs_editor::{Project, packing, pictures};
use wprs_ktx2::TARGETS;

const COVER: &str = "current or previous makes the picture the cover of what the media player plays or played before; none makes it a plain picture again";

#[derive(Subcommand, Debug)]
pub enum Texturing {
    #[command(
        about = "Show a picture asset and what it takes on graphics cards; --ktx2 turns it into a KTX2 texture"
    )]
    Asset {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Asset id")]
        asset: String,
        #[arg(long, help = "Turn the picture into a KTX2 texture with Basis Universal, as an undoable step")]
        ktx2: bool,
        #[arg(long, requires = "ktx2", value_parser = ["uastc", "etc1s"], help = "uastc, sharper (default), or etc1s, smaller")]
        codec: Option<String>,
        #[arg(long, requires = "ktx2", help = "From 1 to 100; 80 when omitted")]
        quality: Option<u32>,
        #[arg(
            long,
            requires = "ktx2",
            help = "Only report the sizes and what each class of graphics card gets"
        )]
        dry_run: bool,
        #[arg(
            long,
            requires = "dry_run",
            value_name = "FOLDER",
            help = "Write a crop of the picture as each class shows it"
        )]
        looks: Option<PathBuf>,
        #[arg(long, conflicts_with = "ktx2", value_parser = wprs_editor::covers::KINDS, help = COVER)]
        cover: Option<String>,
        #[arg(
            long,
            conflicts_with = "ktx2",
            help = "Picture shown while nothing plays: an asset id or a picture of the folder"
        )]
        placeholder: Option<String>,
        #[arg(
            long,
            requires = "cover",
            value_name = "POINTER",
            help = "Show that cover in this field, such as /scene/layers/0/image, in one step: a cover asset of that kind is used again, or a new one is made, and a plain picture never becomes a cover"
        )]
        field: Option<String>,
        #[arg(long, help = crate::cli::JSON)]
        json: bool,
    },
    #[command(
        about = "Send what an adjustment layer captures to RGB lighting, or stop, and choose its region"
    )]
    Rgb {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of an adjustment layer")]
        layer: String,
        #[arg(long, action = ArgAction::Set, help = "true sends and sets permissions.rgb, false stops")]
        on: Option<bool>,
        #[arg(
            long,
            value_name = "WxH",
            conflicts_with = "whole",
            help = "Capture this region around the layer"
        )]
        size: Option<String>,
        #[arg(long, help = "Capture the whole canvas")]
        whole: bool,
    },
}

fn shown(dir: &std::path::Path, doc: &Value, id: &str) -> Result<Value> {
    let asset = doc.pointer(&format!("/scene/assets/{}", wprs_editor::ops::escape(id)));
    let path = asset.and_then(|a| a["path"].as_str()).ok_or_else(|| anyhow!("there is no asset {id}"))?;
    let file = dir.join(path);
    let size = std::fs::metadata(&file)?.len();
    let (w, h) = pictures::dimensions(&file).map_err(|e| anyhow!("the picture cannot be read: {e}"))?;
    let packed = std::fs::read(&file).ok().and_then(|b| wprs_ktx2::info(&b).ok());
    let mips = asset.is_some_and(|a| a["mipmaps"] != false);
    let gpu = |t: wprs_ktx2::Target| match &packed {
        Some(info) => wprs_ktx2::gpu_bytes(info, t, mips),
        None => u64::from(w) * u64::from(h) * 4,
    };
    let classes = TARGETS.map(|t| json!({ "target": t.name(), "class": t.class(), "gpu": gpu(t) }));
    Ok(json!({ "asset": id, "path": path, "bytes": size, "size": [w, h], "codec": packed.map(|i| i.codec),
        "levels": packed.map(|i| i.levels), "hdr": packed.map(|i| i.hdr), "frames": packed.map(|i| i.frames),
        "cube": packed.map(|i| i.cube), "classes": classes }))
}

fn told(v: &Value) -> String {
    let classes = v["classes"].as_array().into_iter().flatten().map(|c| {
        let by = match c["drawn"].as_str() {
            Some("gpu") => " (crop drawn by this card)",
            Some("cpu") => " (crop decoded on the processor)",
            _ => "",
        };
        format!(
            "{} {} {}{by}",
            c["class"].as_str().unwrap_or("?"),
            c["target"].as_str().unwrap_or("?"),
            bytes(c["gpu"].as_u64().unwrap_or(0))
        )
    });
    let shape = match (v["cube"] == true, v["frames"].as_u64().unwrap_or(1)) {
        (true, _) => ", a cube map shown as a panorama".to_string(),
        (_, 2..) => format!(", {} frames in a grid", v["frames"]),
        _ => String::new(),
    };
    let codec = match v["codec"].as_str() {
        Some(c) => format!(", KTX2 {c} with {} levels{shape}", v["levels"]),
        None => String::new(),
    };
    let size = format!("{}x{}", v["size"][0], v["size"][1]);
    let file = bytes(v["bytes"].as_u64().or(v["after"].as_u64()).unwrap_or(0));
    let mut text = format!(
        "{}: {}, {file}, {size}{codec}\n",
        v["asset"].as_str().unwrap_or("?"),
        v["path"].as_str().unwrap_or("?")
    );
    if let Some(before) = v["before"].as_u64() {
        text += &format!(
            "as KTX2 {}: {} before, {} after\n",
            v["codec"].as_str().unwrap_or("?"),
            bytes(before),
            file
        );
    }
    text + "on the graphics card: " + &classes.collect::<Vec<_>>().join(", ")
}

pub fn run(command: Texturing) -> Result<()> {
    let (dir, asset, ktx2, codec, quality, dry_run, looks, json, cover, placeholder, field) = match command {
        Texturing::Asset {
            dir,
            asset,
            ktx2,
            codec,
            quality,
            dry_run,
            looks,
            json,
            cover,
            placeholder,
            field,
        } => (dir, asset, ktx2, codec, quality, dry_run, looks, json, cover, placeholder, field),
        Texturing::Rgb { dir, layer, on, size, whole } => {
            let size = size.map(|s| crate::editing::dimensions(&s)).transpose()?;
            let size = size.map(|(w, h)| [f64::from(w), f64::from(h)]);
            return crate::steps::edit(&dir, Action::Rgb(RgbEdit { layer, on, size, whole }));
        }
    };
    let covered = cover.is_some() || placeholder.is_some();
    let wanted = AssetEdit { asset: asset.clone(), ktx2, codec, quality, cover, placeholder, field };
    if covered {
        return crate::steps::edit(&dir, Action::Asset(wanted));
    }
    let report = match (ktx2, dry_run) {
        (true, false) => return crate::steps::edit(&dir, Action::Asset(wanted)),
        (true, true) => {
            let project = Project::open(&dir)?;
            let renderer = looks.as_ref().and_then(|_| wprs_render::Renderer::shared().ok());
            let gpu = |bytes: &[u8], target| renderer.and_then(|r| r.look_ktx2(bytes, target));
            packing::trial(
                &project.dir,
                &project.doc,
                &wanted,
                looks.as_deref().map(|l| (l, Some(&gpu as packing::Gpu))),
            )?
        }
        _ => shown(&dir, &Project::open(&dir)?.doc, &asset)?,
    };
    println!("{}", if json { report.to_string() } else { told(&report) });
    Ok(())
}
