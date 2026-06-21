pub mod bbtools;
pub mod titles;

use std::env;
use std::fs::create_dir;
use std::path::PathBuf;

use crate::bbtools::*;
use xbe::XBE;

use flate2::bufread::GzDecoder;
use tar::Archive;

const INTERNAL_TAR_BYTES: &[u8] = include_bytes!(env!("INTERNAL_TAR_PATH"));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BBTools version: {}", env!("CARGO_PKG_VERSION"));

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: ./bbtools <game_directory_path>");
        return Ok(());
    }

    let mut game_path = PathBuf::from(args[1].clone());

    // Append the executable file to the path if not present
    if game_path.is_dir() {
        game_path.push("default.xbe");
    }

    if !game_path.is_file() {
        println!("Coult not locate game XBE at: {}", game_path.display());
        return Ok(());
    }

    let mut xbe = XBE::open(&game_path)?;

    if !xbe.is_valid() {
        eprintln!("XBE Hash check failed! Game is either modified or corrupt!");
        return Ok(());
    }

    game_path.pop(); // Adjust the path to point at the root directory containing the game

    println!("Unpacking internal tar...");

    // Unpack the internal tar
    let mut tar_internal = Archive::new(GzDecoder::new(INTERNAL_TAR_BYTES));
    tar_internal.unpack("build/")?;

    let godot_base_path = PathBuf::from("build/godot/");

    // Make proprietary sub directory
    let mut build_path = godot_base_path.clone();
    build_path.push("proprietary/");
    let _ = create_dir(&build_path);

    titles::unpack(&mut xbe, &mut game_path, &mut build_path, &godot_base_path)?;

    let godot_bin_path = PathBuf::from("build/bin/godot");
    titles::build(&mut xbe, &godot_bin_path, &godot_base_path, false)?;

    return Ok(());
}
