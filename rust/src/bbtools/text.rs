use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

use byteorder::LittleEndian;
use byteorder::ReadBytesExt;

use crate::bbtools::xbe::XBE;

pub struct Translation {
    jp: Option<String>,
    en: Option<String>,
}

fn read_utf16_string(mut reader: impl ReadBytesExt) -> Result<String, std::io::Error> {
    let mut runes: Vec<u16> = Vec::with_capacity(64);
    loop {
        let rune = reader.read_u16::<LittleEndian>()?;
        if rune == 0 {
            break;
        }
        runes.push(rune);
    }
    return Ok(String::from_utf16_lossy(&runes));
}

fn sanitize_string(input: &str) -> String {
    // This is a bad way to do this, but the in the grand scheme of things it's not that much of a performance hit'
    input
        .replace("\"", "\"\"")
        .replace(&(0x0Bu8 as char).to_string(), "\\v")
        .replace(&(0x09u8 as char).to_string(), "\\t")
        .replace(&(0x07u8 as char).to_string(), "\\a")
}

pub fn import_translations(
    xbe: &mut XBE,
    section: &str,
    offset: u32,
    count: usize,
) -> Result<Vec<Translation>, std::io::Error> {
    xbe.seek_section_offset(section, offset)?;

    let mut string_offsets: Vec<u32> = vec![0; count * 2];
    xbe.reader
        .read_u32_into::<LittleEndian>(&mut string_offsets)?;

    let mut translations: Vec<Translation> = Vec::with_capacity(count);
    for i in 0..count {
        let jp_pointer = string_offsets[i * 2];
        let en_pointer = string_offsets[i * 2 + 1];

        let jp_string = if jp_pointer != 0 {
            xbe.seek_pointer_offset(jp_pointer)?;
            let jp_string = read_utf16_string(&mut xbe.reader)?;
            Some(jp_string)
        } else {
            None
        };

        let en_string = if en_pointer != 0 {
            xbe.seek_pointer_offset(en_pointer)?;
            let en_string = read_utf16_string(&mut xbe.reader)?;
            Some(en_string)
        } else {
            None
        };

        translations.push(Translation {
            jp: jp_string,
            en: en_string,
        });
    }

    return Ok(translations);
}

pub fn write_translations_to_csv(
    path: &Path,
    prefix: &str,
    translations: &[Translation],
) -> Result<(), std::io::Error> {
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(b"keys,ja,en\n")?;

    for (i, translation) in translations.iter().enumerate() {
        if translation.jp.is_none() && translation.en.is_none() {
            continue;
        }

        let jp_ref = translation.jp.as_ref();
        let en_ref = translation.en.as_ref();

        let jp_str = jp_ref.unwrap_or_else(|| en_ref.unwrap());
        let en_str = en_ref.unwrap_or_else(|| jp_ref.unwrap());

        let jp_sanitized = sanitize_string(jp_str);
        let en_sanitized = sanitize_string(en_str);

        let entry = format!(
            "{}{:04},\"{}\",\"{}\"\n",
            prefix, i, jp_sanitized, en_sanitized
        );
        writer.write_all(entry.as_bytes())?;
    }

    return Ok(());
}
