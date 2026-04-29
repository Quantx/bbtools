pub mod loc;
pub mod sb;

use std::path::{Path, PathBuf};

use crate::bbtools::*;
use xbe::XBE;

pub type FnIndexToPath = fn(usize) -> PathBuf;

pub fn unpack(
    xbe: &mut XBE,
    game_path: &mut PathBuf,
    build_path: &mut PathBuf,
    build_base_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    return match xbe.title_id {
        sb::TITLE_ID => sb::unpack(xbe, game_path, build_path, build_base_path),
        loc::TITLE_ID => loc::unpack(xbe, game_path, build_path, build_base_path),
        _ => Err(format!("Unknown title ID {:08X}", xbe.title_id).into()),
    };
}
