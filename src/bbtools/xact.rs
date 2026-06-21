use std::ffi::CStr;
use std::fmt;
use std::fs;
use std::fs::File;
use std::fs::create_dir;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use byteorder::WriteBytesExt;

use crate::bbtools::FileSlice;

/* NOTE on ADPCM:
 *
 * All mentions of ADPCM here do NOT refer to standard IMA ADPCM (format code 0x0011)
 * They instead refer to a proprietary ADPCM format used on the original XBOX (format code 0x0069)
 * More information can be found here: https://xboxdevwiki.net/Xbox_ADPCM
 */

/*** WAV FILE CONSTANTS ***/
const RIFF_MAGIC: [u8; 4] = *b"RIFF";
const WAVE_MAGIC: [u8; 8] = *b"WAVEfmt ";
const DATA_MAGIC: [u8; 4] = *b"data";
const SMPL_MAGIC: [u8; 4] = *b"SMPL";

const SMPL_HEADER_SIZE: usize = 36;
const SMPL_LOOP_SIZE: usize = 24;

enum Codec {
    PCM,
    ADPCM,
    WMA,
    Unknown,
}

impl From<u8> for Codec {
    fn from(codec: u8) -> Self {
        match codec {
            0 => Codec::PCM,
            1 => Codec::ADPCM,
            2 => Codec::WMA,
            _ => Codec::Unknown,
        }
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Codec::PCM => "PCM",
            Codec::ADPCM => "ADPCM",
            Codec::WMA => "WMA",
            _ => "Unknown",
        };
        write!(f, "{}", name)
    }
}

impl Codec {
    fn wave_format_code(&self) -> u16 {
        match self {
            Codec::PCM => 0x0001,
            Codec::ADPCM => 0x0069,
            _ => 0,
        }
    }

    fn extension(&self) -> &str {
        match self {
            Codec::PCM => "wav",
            Codec::ADPCM => "xwav",
            Codec::WMA => "wma",
            _ => "",
        }
    }
}

const TRACK_SIZE: usize = 24;
pub struct Track {
    name: Option<String>,

    flags: u8,
    duration: u32,

    codec: Codec,
    channels: u16,
    samples_per_sec: u32,
    bits_per_sample: u16,

    data: Vec<u8>,
    loop_region: FileSlice,
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name_unknown = String::from("Unknown Name");
        write!(
            f,
            "Codec {:>5}, Channels {}, Rate {}, Bit Count {:02}, Loop [{}..{}] | {}",
            self.codec.to_string(),
            self.channels,
            self.samples_per_sec,
            self.bits_per_sample,
            self.loop_region.offset,
            self.loop_region.get_end(),
            self.name.as_ref().get_or_insert(&name_unknown).as_str()
        )
    }
}

impl Track {
    fn get_block_alignment(&self) -> u16 {
        self.channels
            * match self.codec {
                Codec::PCM => self.bits_per_sample / 8,
                Codec::ADPCM => self.bits_per_sample * 9, // 36
                _ => 0,
            }
    }

    fn get_avg_bytes_per_sec(&self) -> u32 {
        match self.codec {
            Codec::PCM => self.get_block_alignment() as u32 * self.samples_per_sec,
            Codec::ADPCM => {
                let a = self.get_block_alignment() as i32;
                let c = self.channels as i32;
                let dw = (((a - (7 * c)) * 8) / (4 * c)) + 2;
                assert!(dw > 0, "DW {}, Align {}, Chan {}", dw, a, c);
                (self.samples_per_sec / dw as u32) * self.get_block_alignment() as u32
            }
            _ => 0,
        }
    }

    fn get_wave_header(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(20);
        buf.resize(16, 0);

        LittleEndian::write_u16(&mut buf[0..2], self.codec.wave_format_code());
        LittleEndian::write_u16(&mut buf[2..4], self.channels);
        LittleEndian::write_u32(&mut buf[4..8], self.samples_per_sec);
        LittleEndian::write_u32(&mut buf[8..12], self.get_avg_bytes_per_sec());
        LittleEndian::write_u16(&mut buf[12..14], self.get_block_alignment());
        LittleEndian::write_u16(&mut buf[14..16], self.bits_per_sample);

        if matches!(self.codec, Codec::ADPCM) {
            buf.resize(20, 0);

            LittleEndian::write_u16(&mut buf[16..18], 2); // Extra size
            LittleEndian::write_u16(&mut buf[16..18], 64); // Nibbles per block
        }

        return buf;
    }

    fn get_smpl_block(&self) -> Vec<u8> {
        if self.loop_region.length == 0 {
            return Vec::new();
        }

        let mut buf: Vec<u8> = vec![0; SMPL_HEADER_SIZE + SMPL_LOOP_SIZE];

        // Header
        let header_buf = &mut buf[..SMPL_HEADER_SIZE];

        LittleEndian::write_u32(&mut header_buf[12..16], 1000000000 / self.samples_per_sec); // sample_period
        LittleEndian::write_u32(&mut header_buf[28..32], 1); // loop_count

        // Loop data
        let loop_buf = &mut buf[SMPL_HEADER_SIZE..];

        LittleEndian::write_u32(&mut loop_buf[8..12], self.loop_region.offset as u32);
        LittleEndian::write_u32(&mut loop_buf[12..16], self.loop_region.get_end() as u32);

        return buf;
    }

    fn write(&self, mut writer: impl Write) -> Result<usize, std::io::Error> {
        if matches!(self.codec, Codec::WMA) {
            // WMAs are written as-is
            writer.write_all(&self.data)?;
            return Ok(self.data.len());
        }

        let mut bytes_written: usize = 0;

        let wave_header = self.get_wave_header();
        let smpl_block = self.get_smpl_block();

        let wav_base_size: usize =
            WAVE_MAGIC.len() + 4 + wave_header.len() + DATA_MAGIC.len() + 4 + self.data.len();

        let wav_size: usize = if smpl_block.is_empty() {
            wav_base_size
        } else {
            wav_base_size + SMPL_MAGIC.len() + 4 + smpl_block.len()
        };

        writer.write_all(&RIFF_MAGIC)?;
        writer.write_u32::<LittleEndian>(wav_size as u32)?;
        bytes_written += RIFF_MAGIC.len() + 4;

        writer.write_all(&WAVE_MAGIC)?;
        writer.write_u32::<LittleEndian>(wave_header.len() as u32)?;
        bytes_written += WAVE_MAGIC.len() + 4;

        writer.write_all(&wave_header)?;
        bytes_written += wave_header.len();

        writer.write_all(&DATA_MAGIC)?;
        writer.write_u32::<LittleEndian>(self.data.len() as u32)?;
        bytes_written += DATA_MAGIC.len() + 4;

        writer.write_all(&self.data)?;
        bytes_written += self.data.len();

        if !smpl_block.is_empty() {
            writer.write_all(&SMPL_MAGIC)?;
            writer.write_u32::<LittleEndian>(smpl_block.len() as u32)?;
            bytes_written += SMPL_MAGIC.len() + 4;

            writer.write_all(&smpl_block)?;
            bytes_written += smpl_block.len();
        }

        return Ok(bytes_written);
    }
}

// Xbox Wave Bank
const XWB_MAGIC: [u8; 4] = *b"WBND";
pub struct XWB {
    name: String,
    tracks: Vec<Track>,
}

impl XWB {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let buf = fs::read(path)?;

        let magic = &buf[0..4];
        if magic != XWB_MAGIC {
            return Err("BadMagic".into());
        }

        let version = LittleEndian::read_u32(&buf[4..8]);
        if version != 3 {
            return Err("BadVersion".into());
        }

        let wavebank_file = FileSlice::from(&buf[8..16]);
        let track_header_file = FileSlice::from(&buf[16..24]);
        let _unknown_file = FileSlice::from(&buf[24..32]); // Always zero?
        let track_data_file = FileSlice::from(&buf[32..40]);

        /*** READ WAVEBANK FILE ***/
        let wavebank_buf = &buf[wavebank_file.as_range()];
        let wavebank_flags = LittleEndian::read_u32(&wavebank_buf[0..4]);
        if wavebank_flags != 0 && wavebank_flags != 1 {
            return Err("UnknownFlags".into());
        }

        let track_count = LittleEndian::read_u32(&wavebank_buf[4..8]) as usize;
        let bank_name_cstr =
            CStr::from_bytes_until_nul(&wavebank_buf[8..24]).expect("Failed to get XWB name");
        let bank_name = bank_name_cstr.to_string_lossy().into_owned();

        let track_header_size = LittleEndian::read_u32(&wavebank_buf[24..28]) as usize;
        if track_header_size != TRACK_SIZE {
            return Err("TrackHeaderSizeIncorrect".into());
        }

        let track_name_size = LittleEndian::read_u32(&wavebank_buf[28..32]);
        let alignment = LittleEndian::read_u32(&wavebank_buf[32..36]);

        let tracks_header_buf = &buf[track_header_file.as_range()];
        let tracks_data_buf = &buf[track_data_file.as_range()];

        if track_count * TRACK_SIZE != tracks_header_buf.len() {
            return Err("TrackCountIncorrect".into());
        }

        let (track_list, []) = tracks_header_buf.as_chunks::<TRACK_SIZE>() else {
            panic!("tracks_header_buf length not a multiple of TRACK_SIZE");
        };

        let mut tracks: Vec<Track> = Vec::with_capacity(track_count);
        for track_buf in track_list {
            let flags_duration = LittleEndian::read_u32(&track_buf[0..4]);
            let duration = flags_duration & 0xFFFFFFF; // 28 bits
            let flags = (flags_duration >> 28) & 0xF; // 4 bits

            let format_bits = LittleEndian::read_u32(&track_buf[4..8]);
            let codec = Codec::from(format_bits as u8 & 0x3); // 2 bits
            let channels = (format_bits >> 2) & 0x7; // 3 bits
            let samples_per_sec = (format_bits >> 5) & 0x3FFFF; // 18 bits
            let block_alignment = (format_bits >> 23) & 0xFF; // 8 bits
            let bits_per_sample = if matches!(codec, Codec::ADPCM) {
                4
            } else if (format_bits >> 31) & 0x1 == 0x1 {
                16
            } else {
                8
            };

            let play_region = FileSlice::from(&track_buf[8..16]);

            let data = &tracks_data_buf[play_region.as_range()];

            assert!(block_alignment == 0);

            tracks.push(Track {
                name: None,

                flags: flags as u8,
                duration: duration,

                codec: codec,
                channels: channels as u16,
                samples_per_sec: samples_per_sec,
                bits_per_sample: bits_per_sample as u16,

                data: Vec::from(data),
                loop_region: FileSlice::from(&track_buf[16..24]),
            });
        }

        return Ok(XWB {
            name: bank_name,
            tracks: tracks,
        });
    }

    pub fn export(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        println!("Exporting wavebank: {}", self.name);

        let mut xwb_path = PathBuf::from(path);

        xwb_path.push(self.name.as_str());

        // Make a directory to hold the exported files
        let _ = create_dir(&xwb_path);

        for (i, track) in self.tracks.iter().enumerate() {
            let default_name = format!("track_{}", i);
            let name = track.name.as_ref().get_or_insert(&default_name).as_str();

            xwb_path.push(name);
            xwb_path.set_extension(track.codec.extension());

            let file = File::create(&xwb_path)?;
            let writer = BufWriter::new(file);
            println!("  Track {:03} | {:5}", i, track);
            track.write(writer)?;

            xwb_path.pop();
        }

        return Ok(());
    }
}

// Format reference: https://wiki.xoreos.org/index.php?title=Binary_XACT_SoundBank
const PARAM_3D_SIZE: usize = 40;
pub struct Param3D {
    inside_cone_angle: i16,
    outside_cone_angle: i16,
    outside_cone_volume: i16,
    unknown0: i16,

    minimum_distance: f32,
    maximum_distance: f32,
    distance_factor: f32,
    rolloff_factor: f32,
    doppler_factor: f32,

    unknown1: i32,
    unknown2: i32,
    unknown3: i32,
}

// All possible events are provided for completeness but only "Play" and "EnvelopeAmplitude" are used
enum EventType {
    Play {
        loop_count: usize,
        track: usize,
        bank: usize,
    },
    PlayComplex,
    Stop,
    Pitch,
    Volume,
    LowPass,
    PitchLFO,
    MultiLFO,
    EnvelopeAmplitude {
        unknown0: u16,
        delay: u16,   // In seconds
        attack: u16,  // In seconds
        hold: u16,    // In seconds
        decay: u16,   // In seconds
        release: u16, // In seconds
        sustain: u8,
        unknown1: u8,
    },
    EnvelopePitch,
    Loop,
    Marker,
    Disabled,
    EnvironmentReverb,
    MixBinSpan,

    Unknown,
}

impl EventType {
    fn import(cmd: u8, flags: u8, buf: &[u8]) -> Self {
        match cmd {
            0x00 => {
                assert!(flags & 0x04 == 0); // There are no complex play commands
                assert!(buf.len() == 6);
                EventType::Play {
                    loop_count: LittleEndian::read_u16(&buf[0..2]) as usize,
                    track: LittleEndian::read_u16(&buf[2..4]) as usize,
                    bank: LittleEndian::read_u16(&buf[4..6]) as usize,
                }
            }
            0x0A => {
                assert!(buf.len() == 14);
                EventType::EnvelopeAmplitude {
                    unknown0: LittleEndian::read_u16(&buf[0..2]),
                    delay: LittleEndian::read_u16(&buf[2..4]),
                    attack: LittleEndian::read_u16(&buf[4..6]),
                    hold: LittleEndian::read_u16(&buf[6..8]),
                    decay: LittleEndian::read_u16(&buf[8..10]),
                    release: LittleEndian::read_u16(&buf[10..12]),
                    sustain: buf[12],
                    unknown1: buf[13],
                }
            }
            _ => unimplemented! {},
        }
    }
}

struct Event {
    timestamp: u32, // (Really a u24) Timestamp in milliseconds
    flags: u8,
    event: EventType,
}

impl Event {
    fn import(buf: &[u8], command_header_offset: usize) -> Vec<Self> {
        let offset_count =
            LittleEndian::read_u32(&buf[command_header_offset..command_header_offset + 4]);
        let count = (offset_count & 0xFF) as usize;
        let offset = (offset_count >> 8) as usize;

        let mut cmd_ptr = &buf[offset..];

        let mut events: Vec<Self> = Vec::with_capacity(count);
        for _ in 0..count {
            let cmd = cmd_ptr[0];
            let timestamp = LittleEndian::read_u24(&cmd_ptr[1..4]);
            let param_size = cmd_ptr[4] as usize + 2; // For some reason arg size is off by 2?
            let flags = cmd_ptr[5];

            let param_end = param_size + 6; // Header size is 6
            let event_type = EventType::import(cmd, flags, &cmd_ptr[6..param_end]);

            events.push(Event {
                timestamp: timestamp,
                flags: flags,
                event: event_type,
            });

            cmd_ptr = &cmd_ptr[param_end..];
        }

        return events;
    }
}

const SOUND_SIZE: usize = 20;
pub struct Sound {
    bank: usize,
    track: usize,

    events: Vec<Event>,

    volume: f32,
    lfe: f32,
    pitch: i16,
    track_count: u8,
    layer: i8,
    category: u8,
    flags: u8,
    param3d: usize,
    priority: i8,
    i3dl2_volume: u8,
    eq_gain: u16,
    eq_freq: u16,
}

const CUE_SIZE: usize = 20;
pub struct Cue {
    flags: u16,
    sound: usize,
    name: String,
    //variations: u32,
    xfade: u16,
    unknown: u16,
    transitions: u32,
}

// Xbox Sound Bank
const XSB_MAGIC: [u8; 4] = *b"SDBK";
const XSB_HEADER_SIZE: usize = 56;
pub struct XSB {
    name: String,
    banks: Vec<XWB>,
    param3ds: Vec<Param3D>,
    sounds: Vec<Sound>,
    cues: Vec<Cue>,
}

impl XSB {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let buf = fs::read(path)?;

        let magic = &buf[0..4];
        if magic != XSB_MAGIC {
            return Err("BadMagic".into());
        }

        let version = LittleEndian::read_u16(&buf[4..6]);
        if version != 11 {
            return Err("UnsupportedVersion".into());
        }

        let strings_offset = LittleEndian::read_u32(&buf[8..12]) as usize;
        let crossfade_offset = LittleEndian::read_u32(&buf[12..16]) as usize;
        let param3d_offset = LittleEndian::read_u32(&buf[16..20]) as usize;
        let unknown_offset = LittleEndian::read_u32(&buf[20..24]) as usize;

        let _unknown0_count = LittleEndian::read_u16(&buf[26..28]) as usize;
        let sound_count = LittleEndian::read_u16(&buf[28..30]) as usize;
        let cue_count = LittleEndian::read_u16(&buf[30..32]) as usize;
        let _unknown1_count = LittleEndian::read_u16(&buf[32..34]) as usize;
        let bank_count = LittleEndian::read_u16(&buf[34..36]) as usize;

        assert!((crossfade_offset - param3d_offset) % PARAM_3D_SIZE == 0);
        let param3d_count = (crossfade_offset - param3d_offset) / PARAM_3D_SIZE;

        println!(
            "Counts: Unk0 {}, Sounds {}, Cues {}, Unk1 {}, Banks {}, Param3Ds {}",
            _unknown0_count, sound_count, cue_count, _unknown1_count, bank_count, param3d_count
        );

        let xsb_name_cstr =
            CStr::from_bytes_until_nul(&buf[40..56]).expect("Failed to get XSB name");
        let xsb_name = xsb_name_cstr.to_string_lossy().into_owned();

        let mut xsb = XSB {
            name: xsb_name,
            banks: Vec::with_capacity(bank_count),
            param3ds: Vec::with_capacity(param3d_count),
            sounds: Vec::with_capacity(sound_count),
            cues: Vec::with_capacity(cue_count),
        };

        /*** IMPORT WAVEBANKS ***/
        let bank_names_buf_end = strings_offset + 16 * bank_count;
        let bank_names_buf = &buf[strings_offset..bank_names_buf_end];

        let (bank_names_list, []) = bank_names_buf.as_chunks::<16>() else {
            panic!("bank_names_buf length not a multiple of 16");
        };

        let mut root_path = PathBuf::from(path);
        root_path.pop(); // Drop "Bank.xsb" from path

        for bank_name_buf in bank_names_list {
            let bank_name_cstr = CStr::from_bytes_until_nul(bank_name_buf)
                .expect("Failed to get Bank name from string table");
            let mut bank_name = String::from(bank_name_cstr.to_str().unwrap());
            bank_name += ".xwb";

            let mut xwb_path = root_path.clone();
            xwb_path.push(&bank_name);

            if !xwb_path.is_file() {
                // File could not be located, preform a case-insensitive search for it
                bank_name.make_ascii_lowercase();
                if let Ok(entries) = fs::read_dir(&root_path) {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let entry_path = entry.path();
                            if !entry_path.is_file() {
                                continue;
                            }

                            let entry_name = entry_path.file_name().unwrap().to_ascii_lowercase();

                            if *bank_name == *entry_name {
                                xwb_path = PathBuf::from(entry_path);
                                break;
                            }
                        }
                    }
                }
            }

            if !xwb_path.is_file() {
                return Err(format!("Could not locate WaveBank {}", bank_name).into());
            }

            let xwb = XWB::open(&xwb_path)?;
            xsb.banks.push(xwb);
        }

        /*** IMPORT PARAM3D ***/
        let param3ds_buf = &buf[param3d_offset..crossfade_offset];

        let (param3d_list, []) = param3ds_buf.as_chunks::<PARAM_3D_SIZE>() else {
            panic!("param3ds_buf length not a multiple of PARAM_3D_SIZE");
        };

        for param3d_buf in param3d_list {
            xsb.param3ds.push(Param3D {
                inside_cone_angle: LittleEndian::read_i16(&param3d_buf[0..2]),
                outside_cone_angle: LittleEndian::read_i16(&param3d_buf[2..4]),
                outside_cone_volume: LittleEndian::read_i16(&param3d_buf[4..6]),
                unknown0: LittleEndian::read_i16(&param3d_buf[6..8]),

                minimum_distance: LittleEndian::read_f32(&param3d_buf[8..12]),
                maximum_distance: LittleEndian::read_f32(&param3d_buf[12..16]),
                distance_factor: LittleEndian::read_f32(&param3d_buf[16..20]),
                rolloff_factor: LittleEndian::read_f32(&param3d_buf[20..24]),
                doppler_factor: LittleEndian::read_f32(&param3d_buf[24..28]),

                unknown1: LittleEndian::read_i32(&param3d_buf[28..32]),
                unknown2: LittleEndian::read_i32(&param3d_buf[32..36]),
                unknown3: LittleEndian::read_i32(&param3d_buf[36..40]),
            });
        }

        let cues_buf_end = XSB_HEADER_SIZE + cue_count * CUE_SIZE;
        let cues_buf = &buf[XSB_HEADER_SIZE..cues_buf_end];

        let sounds_buf_end = cues_buf_end + sound_count * SOUND_SIZE;
        let sounds_buf = &buf[cues_buf_end..sounds_buf_end];

        /*** IMPORT SOUNDS ***/
        let (sound_list, []) = sounds_buf.as_chunks::<SOUND_SIZE>() else {
            panic!("sounds_buf length not a multiple of SOUND_SIZE");
        };

        for sound_buf in sound_list {
            let lfe_volume = LittleEndian::read_u16(&sound_buf[4..6]);
            let volume = -0.16 * (lfe_volume & 0x1FF) as f32; // 9 bits
            let lfe = -0.5 * ((lfe_volume >> 9) & 0x7F) as f32; // 7 bits

            let flags = sound_buf[11];

            let bank: usize;
            let track: usize;
            let events: Vec<Event>;

            if flags & 0x8 == 0x8 {
                track = LittleEndian::read_u16(&sound_buf[0..2]) as usize;
                bank = LittleEndian::read_u16(&sound_buf[2..4]) as usize;
                events = Vec::new();
            } else {
                let cmd_offset = LittleEndian::read_u32(&sound_buf[0..4]) as usize;

                events = Event::import(&buf, cmd_offset);

                if let Some(last_event) = events.iter().last() {
                    if let EventType::Play {
                        loop_count: _,
                        track: t,
                        bank: b,
                    } = last_event.event
                    {
                        bank = b;
                        track = t;
                    } else {
                        return Err("Last event in CUE was not a play event".into());
                    }
                } else {
                    return Err("CUE is complex but contains no events".into());
                };
            }

            xsb.sounds.push(Sound {
                bank: bank,
                track: track,
                events: events,

                volume: volume,
                lfe: lfe,
                pitch: LittleEndian::read_i16(&sound_buf[6..8]),
                track_count: sound_buf[8],
                layer: sound_buf[9] as i8,
                category: sound_buf[10],
                flags: flags,
                param3d: LittleEndian::read_u16(&sound_buf[12..14]) as usize,
                priority: sound_buf[14] as i8,
                i3dl2_volume: sound_buf[15],
                eq_gain: LittleEndian::read_u16(&sound_buf[16..18]),
                eq_freq: LittleEndian::read_u16(&sound_buf[18..20]),
            });
        }

        /*** IMPORT CUES ***/
        let (cue_list, []) = cues_buf.as_chunks::<CUE_SIZE>() else {
            panic!("cues_buf length not a multiple of CUE_SIZE");
        };

        for cue_buf in cue_list {
            let cue_name_offset = LittleEndian::read_u32(&cue_buf[4..8]) as usize;
            let cue_name_cstr = CStr::from_bytes_until_nul(&buf[cue_name_offset..])
                .expect("Failed to get Cue name");
            let cue_name = cue_name_cstr.to_string_lossy().into_owned();

            let variations = LittleEndian::read_u32(&cue_buf[8..12]);
            assert!(variations == u32::MAX);

            xsb.cues.push(Cue {
                flags: LittleEndian::read_u16(&cue_buf[0..2]),
                sound: LittleEndian::read_u16(&cue_buf[2..4]) as usize,
                name: cue_name,
                xfade: LittleEndian::read_u16(&cue_buf[12..14]),
                unknown: LittleEndian::read_u16(&cue_buf[14..16]),
                transitions: LittleEndian::read_u32(&cue_buf[16..20]),
            });
        }

        /*** Transfer CUE names to Track names ***/
        for cue in &xsb.cues {
            let sound = &xsb.sounds[cue.sound];

            let bank = &mut xsb.banks[sound.bank];
            let track = &mut bank.tracks[sound.track];

            // Prefer shorter names when possible
            if track.name.is_none() || cue.name.len() < track.name.as_ref().unwrap().len() {
                track.name = Some(cue.name.clone());
            }
        }

        return Ok(xsb);
    }

    pub fn export_banks(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        for bank in &self.banks {
            bank.export(path)?;
        }

        return Ok(());
    }
}
