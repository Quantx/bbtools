pub mod loc;
pub mod sb;

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bbtools::*;
use xbe::XBE;

pub fn extract_iso(extract_path: &mut PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let iso_path = extract_path.clone();
    extract_path.set_extension("");

    let mut xiso_extract_cmd = Command::new("build/bin/extract-xiso");
    xiso_extract_cmd.arg(iso_path).arg("-d").arg(&extract_path);
    xiso_extract_cmd.status()?; // Extract file system

    extract_path.push("default.xbe");
    return Ok(());
}

pub fn unpack(
    xbe: &mut XBE,
    game_path: &mut PathBuf,
    build_path: &mut PathBuf,
    godot_base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match xbe.title_id {
        sb::TITLE_ID => sb::unpack(xbe, game_path, build_path, godot_base_path),
        loc::TITLE_ID => loc::unpack(xbe, game_path, build_path, godot_base_path),
        _ => Err(format!("Unknown title ID {:08X}", xbe.title_id).into()),
    }
}

pub fn build(
    xbe: &mut XBE,
    godot_bin_path: &Path,
    godot_base_path: &Path,
    as_zip: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let title_prefix = match xbe.title_id {
        sb::TITLE_ID => sb::TITLE_PREFIX,
        loc::TITLE_ID => loc::TITLE_PREFIX,
        _ => return Err(format!("Unknown title ID {:08X}", xbe.title_id).into()),
    };

    let export_extension = if as_zip { "zip" } else { "pck" };

    let mut export_path = PathBuf::from("..");
    export_path.push(title_prefix);
    export_path.set_extension(export_extension);

    let mut godot_import_cmd = Command::new(godot_bin_path);
    godot_import_cmd
        .arg("--headless")
        .arg("--path")
        .arg(godot_base_path)
        .arg("--import");

    godot_import_cmd.status()?; // Import all assets (first pass)

    let mut godot_export_cmd = Command::new(godot_bin_path);
    godot_export_cmd
        .arg("--headless")
        .arg("--path")
        .arg(godot_base_path)
        .arg("--export-pack")
        .arg(title_prefix)
        .arg(export_path);

    godot_export_cmd.status()?; // Export the project

    return Ok(());
}
