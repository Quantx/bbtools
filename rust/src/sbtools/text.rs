
use crate::sbtools::xbe::XBE;

pub struct Translation {
    en: Option<String>,
    jp: Option<String>,
}


pub fn import_translations(xbe: &mut XBE, section: &str, offset: u32, count: usize) -> Result<Vec<Translation>, std::io::Error> {
    xbe.seek_section(section, offset);

}
