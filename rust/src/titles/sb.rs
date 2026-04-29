use std::fs::create_dir;
use std::path::{Path, PathBuf};

use crate::bbtools::*;
use xbe::XBE;

pub const TITLE_ID: u32 = 0x43430002;
pub fn unpack(
    _xbe: &mut XBE,
    _game_path: &mut PathBuf,
    build_path: &mut PathBuf,
    _build_base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Unpacking SB");

    build_path.push("sb");
    let _ = create_dir(&build_path);

    return Ok(());
}
