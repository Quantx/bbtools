pub mod bbtools;
pub mod titles;

use std::env;
use std::fs::create_dir;
use std::path::PathBuf;

use crate::bbtools::*;
use xbe::XBE;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args();
    if args.len() < 2 {
        println!("Usage: ./bbtools <game_directory_path> <output_directory_path>");
        return Ok(());
    }

    let game_path_str = args.last().expect("Missing path to game directory");
    let mut game_path = PathBuf::from(game_path_str);

    // Append the executable file to the path if not present
    if game_path.is_dir() {
        game_path.push("default.xbe");
    }

    if !game_path.is_file() {
        println!("Coult not locate game XBE at: {}", game_path.display());
        return Ok(());
    }

    let mut xbe = XBE::open(&game_path)?;

    game_path.pop(); // Adjust the path to point at the root directory containing the game

    // Make build directory
    let build_base_path = PathBuf::from("build_rust");
    let _ = create_dir(&build_base_path);

    // Make proprietary sub directory
    let mut build_path = build_base_path.clone();
    build_path.push("proprietary");
    let _ = create_dir(&build_path);

    titles::unpack(&mut xbe, &mut game_path, &mut build_path, &build_base_path)?;

    return Ok(());
}
