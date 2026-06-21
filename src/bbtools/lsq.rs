use std::cmp;
use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use byteorder::WriteBytesExt;

use glam::f32::Vec3A as Vec3; // Vec3A is 16-bytes so that it can function with SIMD

use crate::bbtools::FileSlice;

use pathdiff::diff_paths;

use gltf::json::buffer::*;
use gltf::json::extensions::animation::Animation as AnimationExtension;
use gltf::json::extras::Void;
use gltf::json::validation::*;
use gltf::json::*;

use serde_json::{Map, Number, Value};

const TRACK_INFO_SIZE: usize = 12;
const FRAME_INFO_SIZE: usize = 8;

const EFFECT_SIZE: usize = 80;
pub struct Effect {
    idx: u16,
    jnt: u8,
    flags: u32,
    jnt_delay: f32,
    pos: Vec3,
    rot: Vec3,
    trans_rot: Vec3,
}

impl Effect {
    pub fn import(buf: &[u8; EFFECT_SIZE], scale: f32, spf: f32) -> Self {
        let idx = LittleEndian::read_u32(&buf[0..4]);
        assert!(idx <= u16::MAX as u32);

        let jnt_a = buf[4];
        let jnt_b = buf[5];

        assert!(LittleEndian::read_u16(&buf[6..8]) == 0);

        let flags = LittleEndian::read_u32(&buf[8..12]);

        assert!(LittleEndian::read_u32(&buf[12..16]) == 0);

        let jnt_delay = LittleEndian::read_u32(&buf[16..20]) as f32 * spf;

        let pos = Vec3::new(
            LittleEndian::read_f32(&buf[20..24]),
            LittleEndian::read_f32(&buf[24..28]),
            LittleEndian::read_f32(&buf[28..32]),
        ) * scale;

        let rot = Vec3::new(
            LittleEndian::read_f32(&buf[32..36]),
            LittleEndian::read_f32(&buf[36..40]),
            LittleEndian::read_f32(&buf[40..44]),
        );

        let trans_rot = Vec3::new(
            LittleEndian::read_f32(&buf[44..48]),
            LittleEndian::read_f32(&buf[48..52]),
            LittleEndian::read_f32(&buf[52..56]),
        );

        let zero: [u8; 24] = [0; _];
        assert!(zero == buf[56..EFFECT_SIZE]);

        return Effect {
            idx: idx as u16,
            jnt: if flags & 0x8 == 0x8 { jnt_b } else { jnt_a },
            flags: flags,
            jnt_delay: jnt_delay,
            pos: pos,
            rot: rot,
            trans_rot: trans_rot,
        };
    }

    pub fn write_effect(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.idx)?;
        writer.write_u8(self.jnt)?;
        writer.write_u32::<LittleEndian>(self.flags)?;
        writer.write_f32::<LittleEndian>(self.jnt_delay)?;

        writer.write_f32::<LittleEndian>(self.pos.x)?;
        writer.write_f32::<LittleEndian>(self.pos.y)?;
        writer.write_f32::<LittleEndian>(self.pos.z)?;

        writer.write_f32::<LittleEndian>(self.rot.x)?;
        writer.write_f32::<LittleEndian>(self.rot.y)?;
        writer.write_f32::<LittleEndian>(self.rot.z)?;

        writer.write_f32::<LittleEndian>(self.trans_rot.x)?;
        writer.write_f32::<LittleEndian>(self.trans_rot.y)?;
        writer.write_f32::<LittleEndian>(self.trans_rot.z)?;

        return Ok(());
    }
}

const SOUND_SIZE: usize = 12;
pub struct Sound {
    id: u16,
    jnt: u8,
    flags: u32,
}

impl Sound {
    pub fn import(buf: &[u8; SOUND_SIZE]) -> Self {
        let id = LittleEndian::read_u32(&buf[0..4]);
        assert!(id <= u16::MAX as u32);
        let jnt = LittleEndian::read_u32(&buf[4..8]);
        assert!(jnt <= u8::MAX as u32);
        let flags = LittleEndian::read_u32(&buf[8..12]);

        return Sound {
            id: id as u16,
            jnt: jnt as u8,
            flags: flags,
        };
    }

    pub fn write_sound(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u16::<LittleEndian>(self.id)?;
        writer.write_u8(self.jnt)?;
        writer.write_u32::<LittleEndian>(self.flags)?;

        return Ok(());
    }
}

pub enum EventType {
    Effect(Vec<Effect>),
    Sound(Vec<Sound>),
    MotionChangeable,
    Flag(u16),
    Light,
}

pub struct Frame {
    time: f32,
    event: EventType,
}

impl Frame {
    pub fn import(buf: &[u8], import_event_type: &EventType, scale: f32, spf: f32) -> Self {
        let time = LittleEndian::read_u16(&buf[0..2]) as f32 * spf;
        let event_count = LittleEndian::read_u16(&buf[2..4]) as usize;

        let event = match import_event_type {
            EventType::Effect(_) => {
                let effects_buf_end = 4 + event_count * EFFECT_SIZE;
                let effects_buf = &buf[4..effects_buf_end];

                let (effects_list, []) = effects_buf.as_chunks::<EFFECT_SIZE>() else {
                    panic!("effects_buf length not a multiple of EFFECT_SIZE");
                };

                let mut effects: Vec<Effect> = Vec::with_capacity(event_count);
                for effect_buf in effects_list {
                    effects.push(Effect::import(effect_buf, scale, spf));
                }

                EventType::Effect(effects)
            }
            EventType::Sound(_) => {
                let sounds_buf_end = 4 + event_count * SOUND_SIZE;
                let sounds_buf = &buf[4..sounds_buf_end];

                let (sounds_list, []) = sounds_buf.as_chunks::<SOUND_SIZE>() else {
                    panic!("sounds_buf length not a multiple of SOUND_SIZE");
                };

                let mut sounds: Vec<Sound> = Vec::with_capacity(event_count);
                for sound_buf in sounds_list {
                    sounds.push(Sound::import(sound_buf));
                }

                EventType::Sound(sounds)
            }
            EventType::MotionChangeable => EventType::MotionChangeable,
            EventType::Flag(_) => {
                assert!(event_count == 1);

                let flags = LittleEndian::read_u16(&buf[4..6]);

                let zeros: [u8; 13] = [0; _];
                assert!(&buf[6..19] == &zeros);

                EventType::Flag(flags)
            }
            EventType::Light => EventType::Light,
        };

        return Frame {
            time: time,
            event: event,
        };
    }

    pub fn write_frame(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_f32::<LittleEndian>(self.time)?;
        match self.event {
            EventType::Effect(ref effects) => {
                writer.write_u32::<LittleEndian>(effects.len() as u32)?;
                for effect in effects.iter() {
                    effect.write_effect(writer)?;
                }
            }
            EventType::Sound(ref sounds) => {
                writer.write_u32::<LittleEndian>(sounds.len() as u32)?;
                for sound in sounds.iter() {
                    sound.write_sound(writer)?;
                }
            }
            EventType::Flag(flags) => {
                writer.write_u16::<LittleEndian>(flags)?;
            }
            _ => todo!("EventType not yet implemented"),
        }

        return Ok(());
    }

    pub fn size(&self) -> usize {
        4 + (match self.event {
            EventType::Effect(ref effects) => 4 + effects.len() * 47,
            EventType::Sound(ref sounds) => 4 + sounds.len() * 7,
            EventType::Flag(_) => 2,
            _ => todo!("EventType not yet implemented"),
        })
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Time: {}", self.time)?;
        match self.event {
            EventType::Effect(ref effects) => {
                for (i, effect) in effects.iter().enumerate() {
                    writeln!(
                        f,
                        "      Effect {:02} - IDX {:04}, JNT {:02}, Flags {:08X}, JNT Delay {}, Pos {:.4}, Rot {:.4}, Trans Rot {:.4}",
                        i,
                        effect.idx,
                        effect.jnt,
                        effect.flags,
                        effect.jnt_delay,
                        effect.pos,
                        effect.rot,
                        effect.trans_rot
                    )?;
                }
            }
            EventType::Sound(ref sounds) => {
                for (i, sound) in sounds.iter().enumerate() {
                    writeln!(
                        f,
                        "      Sound {:02} - ID {:04}, JNT {:02}, Flags {:08X}",
                        i, sound.id, sound.jnt, sound.flags
                    )?;
                }
            }
            EventType::Flag(flags) => {
                writeln!(f, "      Flags {:04X}", flags)?;
            }
            _ => todo!("EventType not yet implemented"),
        }

        return Ok(());
    }
}

pub struct Track {
    animation_id: u32,
    frames: Vec<Frame>,
}

impl Track {
    pub fn import_tracks(
        buf: &[u8],
        import_event_type: EventType,
        scale: f32,
        spf: f32,
    ) -> Vec<Self> {
        let track_count = buf[1] as usize;

        let mut track_files: Vec<(FileSlice, u32)> = Vec::with_capacity(track_count);

        let track_info_buf_end = 4 + track_count * TRACK_INFO_SIZE;
        let track_info_buf = &buf[4..track_info_buf_end];

        let (track_info_list, []) = track_info_buf.as_chunks::<TRACK_INFO_SIZE>() else {
            panic!("track_info_buf length not a multiple of TRACK_INFO_SIZE");
        };

        for track_info in track_info_list {
            track_files.push((
                FileSlice::from(&track_info[0..8]),
                LittleEndian::read_u32(&track_info[8..12]),
            ));
        }

        let mut tracks: Vec<Track> = Vec::with_capacity(track_count);
        for (track_file, animation_id) in track_files.iter() {
            let track_buf = &buf[track_file.as_range()];

            let frame_count = LittleEndian::read_u32(&track_buf[0..4]) as usize;

            let frame_info_buf_end = 4 + frame_count * FRAME_INFO_SIZE;
            let frame_info_buf = &track_buf[4..frame_info_buf_end];

            let (frame_info_list, []) = frame_info_buf.as_chunks::<FRAME_INFO_SIZE>() else {
                panic!("frame_info_buf length not a multiple of FRAME_INFO_SIZE");
            };

            let mut frames: Vec<Frame> = Vec::with_capacity(frame_count);
            for frame_info in frame_info_list.iter() {
                let time = LittleEndian::read_u16(&frame_info[0..2]);
                let frame_offset = LittleEndian::read_u32(&frame_info[4..8]) as usize;

                let frame_buf = &track_buf[frame_offset..];
                assert!(time == LittleEndian::read_u16(&frame_buf[0..2])); // The time index is duplicated

                frames.push(Frame::import(frame_buf, &import_event_type, scale, spf));
            }

            tracks.push(Track {
                animation_id: *animation_id,
                frames: frames,
            })
        }

        return tracks;
    }

    pub fn write_track(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        writer.write_u32::<LittleEndian>(self.frames.len() as u32)?;
        for frame in self.frames.iter() {
            frame.write_frame(writer)?;
        }

        return Ok(());
    }

    pub fn size(&self) -> usize {
        4 + self.frames.iter().fold(0, |acc, frame| acc + frame.size())
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Animation ID: {}, Frame Count: {}",
            self.animation_id,
            self.frames.len()
        )?;
        for (i, frame) in self.frames.iter().enumerate() {
            write!(f, "    Frame {:02} - {}", i, frame)?;
        }

        return Ok(());
    }
}

pub struct LSQ {
    pub animation_count: usize,
    effects: Vec<Track>,
    sounds: Vec<Track>,
    flags: Vec<Track>,
    glbin_path: Option<PathBuf>,
}

impl LSQ {
    pub fn import(buf: &[u8], scale: f32, fps: f32) -> Result<Self, Box<dyn std::error::Error>> {
        // No MAGIC for this file type unfortunately
        let spf = 1.0 / fps;

        let effects_offset = LittleEndian::read_u32(&buf[0..4]) as usize;
        let sounds_offset = LittleEndian::read_u32(&buf[4..8]) as usize;
        let motion_changeable_offset = LittleEndian::read_u32(&buf[8..12]) as usize;
        let flags_offset = LittleEndian::read_u32(&buf[12..16]) as usize;
        let lights_offset = LittleEndian::read_u32(&buf[16..20]) as usize;

        assert!(motion_changeable_offset == 0);
        assert!(lights_offset == 0);

        let effects_buf = &buf[effects_offset..];
        let sounds_buf = &buf[sounds_offset..];
        let flags_buf = &buf[flags_offset..];

        // println!("Eff Anim Count {}, Eff Anim Actual {}", effects_buf[0], effects_buf[1]);
        // println!("Snd Anim Count {}, Snd Anim Actual {}", sounds_buf[0], sounds_buf[1]);
        // println!("Flg Anim Count {}, Flg Anim Actual {}", flags_buf[0], flags_buf[1]);

        let effects_animation_count = if effects_buf[1] > 0 {
            effects_buf[0]
        } else {
            0
        };
        let sounds_animation_count = if sounds_buf[1] > 0 { sounds_buf[0] } else { 0 };
        let flags_animation_count = if flags_buf[1] > 0 { flags_buf[0] } else { 0 };

        let animation_count = cmp::max(
            effects_animation_count,
            cmp::max(sounds_animation_count, flags_animation_count),
        );

        if effects_buf[1] > 0 {
            assert!(animation_count == effects_buf[0])
        }
        if sounds_buf[1] > 0 {
            assert!(animation_count == sounds_buf[0])
        }
        if flags_buf[1] > 0 {
            assert!(animation_count == flags_buf[0])
        }

        let effects = Track::import_tracks(effects_buf, EventType::Effect(Vec::new()), scale, spf);
        let sounds = Track::import_tracks(sounds_buf, EventType::Sound(Vec::new()), scale, spf);
        let flags = Track::import_tracks(flags_buf, EventType::Flag(0), scale, spf);

        return Ok(LSQ {
            animation_count: animation_count as usize,
            effects: effects,
            sounds: sounds,
            flags: flags,
            glbin_path: None,
        });
    }

    pub fn path_to_glbin(&self, path: &Path) -> Option<PathBuf> {
        let mut gltf_path = PathBuf::from(path);
        gltf_path.set_file_name("");

        return diff_paths(self.glbin_path.as_ref()?, gltf_path);
    }

    pub fn write_to_glbin(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        for track in self.effects.iter() {
            track.write_track(&mut writer)?;
        }

        for track in self.sounds.iter() {
            track.write_track(&mut writer)?;
        }

        for track in self.flags.iter() {
            track.write_track(&mut writer)?;
        }

        self.glbin_path = Some(path.to_path_buf());

        return Ok(());
    }

    pub fn size(&self) -> usize {
        self.effects.iter().fold(0, |acc, track| acc + track.size())
            + self.sounds.iter().fold(0, |acc, track| acc + track.size())
            + self.flags.iter().fold(0, |acc, track| acc + track.size())
    }

    pub fn append_to_animation(
        animation: &mut Animation,
        track_type_name: &str,
        view_index: usize,
    ) {
        let extensions = animation
            .extensions
            .get_or_insert(AnimationExtension { others: Map::new() });

        if let &mut Value::Object(ref mut bbtools_ext) = extensions
            .others
            .entry("bbtools")
            .or_insert(Value::Object(Map::new()))
        {
            if let &mut Value::Object(ref mut sequence) = bbtools_ext
                .entry("sequence")
                .or_insert(Value::Object(Map::new()))
            {
                assert!(!sequence.contains_key(track_type_name));
                sequence.insert(
                    track_type_name.to_owned(),
                    Value::Number(Number::from(view_index)),
                );
            }
        }
    }

    pub fn append_to_gltf(
        &self,
        root: &mut Root,
        gltf_path: &Path,
        include_mirrored: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let glbin_rel_path = if let Some(p) = self.path_to_glbin(gltf_path) {
            p
        } else {
            return Err("Cannot get relative path to sequence glbin".into());
        };

        let glbin_rel_path_str = glbin_rel_path.to_string_lossy().into_owned();

        let buf_idx = root.push(Buffer {
            name: Some(String::from("Sequence")),
            uri: Some(glbin_rel_path_str),
            byte_length: USize64::from(self.size()),
            extensions: None,
            extras: Void::default(),
        });

        let mut view_offset: usize = 0;
        for (i, track) in self.effects.iter().enumerate() {
            let track_view_size = track.size();
            let track_view = root.push(View {
                name: Some(format!("Sequence {} - Effects", i)),
                buffer: buf_idx,
                byte_length: USize64::from(track_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += track_view_size;

            let animation = &mut root.animations[track.animation_id as usize];
            Self::append_to_animation(animation, "effects", track_view.value());

            if include_mirrored {
                let animation_mirrored =
                    &mut root.animations[track.animation_id as usize + self.animation_count];
                Self::append_to_animation(animation_mirrored, "effects", track_view.value());
            }
        }

        for (i, track) in self.sounds.iter().enumerate() {
            let track_view_size = track.size();
            let track_view = root.push(View {
                name: Some(format!("Sequence {} - Sounds", i)),
                buffer: buf_idx,
                byte_length: USize64::from(track_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += track_view_size;

            let animation = &mut root.animations[track.animation_id as usize];
            Self::append_to_animation(animation, "sounds", track_view.value());

            if include_mirrored {
                let animation_mirrored =
                    &mut root.animations[track.animation_id as usize + self.animation_count];
                Self::append_to_animation(animation_mirrored, "sounds", track_view.value());
            }
        }

        for (i, track) in self.flags.iter().enumerate() {
            let track_view_size = track.size();
            let track_view = root.push(View {
                name: Some(format!("Sequence {} - Flags", i)),
                buffer: buf_idx,
                byte_length: USize64::from(track_view_size),
                byte_offset: Some(USize64::from(view_offset)),
                byte_stride: None,
                extensions: None,
                extras: Extras::default(),
                target: None,
            });
            view_offset += track_view_size;

            let animation = &mut root.animations[track.animation_id as usize];
            Self::append_to_animation(animation, "flags", track_view.value());

            if include_mirrored {
                let animation_mirrored =
                    &mut root.animations[track.animation_id as usize + self.animation_count];
                Self::append_to_animation(animation_mirrored, "flags", track_view.value());
            }
        }

        return Ok(());
    }
}

impl fmt::Display for LSQ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Expected Animation Count {}", self.animation_count)?;

        writeln!(f, "Effect Track Count {}", self.effects.len())?;
        for (i, track) in self.effects.iter().enumerate() {
            write!(f, "  Effect Track {:02} - {}", i, track)?;
        }

        writeln!(f, "Sound Track Count {}", self.sounds.len())?;
        for (i, track) in self.sounds.iter().enumerate() {
            write!(f, "  Sound Track {:02} - {}", i, track)?;
        }

        writeln!(f, "Flags Track Count {}", self.flags.len())?;
        for (i, track) in self.flags.iter().enumerate() {
            write!(f, "  Flag Track {:02} - {}", i, track)?;
        }

        return Ok(());
    }
}
