use std::fmt;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use pathdiff::diff_paths;

use glam::EulerRot;
use glam::Quat;
use glam::Vec3A as Vec3;

use gltf::json::Animation as GltfAnimation;
use gltf::json::accessor::*;
use gltf::json::animation::*;
use gltf::json::buffer::*;
use gltf::json::extensions::animation::Animation as AnimationExtension;
use gltf::json::extras::Void;
use gltf::json::validation::*;
use gltf::json::*;

use serde_json::{Map, json};

use crate::bbtools::lsq::LSQ;

const LMT_FRAME_SIZE: usize = 28;
const LMT_FRAME_STRIDE: usize = 32;
struct Frame {
    time: f32,
    pos: Vec3,
    rot: Vec3,
}

impl Frame {
    fn to_array(&self, mirror: bool) -> [u8; LMT_FRAME_STRIDE] {
        let mut buf: [u8; LMT_FRAME_STRIDE] = [0; _];

        let (pos_x, rot_y, rot_z) = if mirror {
            (-self.pos.x, -self.rot.y, -self.rot.z)
        } else {
            (self.pos.x, self.rot.y, self.rot.z)
        };

        LittleEndian::write_f32(&mut buf[0..4], self.time);

        LittleEndian::write_f32(&mut buf[4..8], pos_x);
        LittleEndian::write_f32(&mut buf[8..12], self.pos.y);
        LittleEndian::write_f32(&mut buf[12..16], self.pos.z);

        let quat = Quat::from_euler(EulerRot::XYZEx, self.rot.x, rot_y, rot_z);

        LittleEndian::write_f32(&mut buf[16..20], quat.x);
        LittleEndian::write_f32(&mut buf[20..24], quat.y);
        LittleEndian::write_f32(&mut buf[24..28], quat.z);
        LittleEndian::write_f32(&mut buf[28..32], quat.w);

        return buf;
    }
}

struct Track {
    node: u32,
    frames: Vec<Frame>,
    time_min: f32,
    time_max: f32,
}

impl Track {
    fn write(&self, mirror: bool, mut writer: impl Write) -> Result<usize, std::io::Error> {
        let mut bytes_written: usize = 0;
        for frame in &self.frames {
            let data = frame.to_array(mirror);
            writer.write_all(data.as_slice())?;
            bytes_written += data.len();
        }

        assert!(bytes_written == self.size());

        return Ok(bytes_written);
    }

    fn size(&self) -> usize {
        self.frames.len() * LMT_FRAME_STRIDE
    }
}

struct Animation {
    offset: usize,
    duration: f32,
    tracks: Vec<Track>,
}

impl Animation {
    fn size(&self) -> usize {
        self.tracks.iter().fold(0, |acc, track| acc + track.size())
    }
}

pub struct LMT<'a> {
    // Maximum number of tracks in any given animation
    // This should match the number of nodes in the model
    track_count_max: usize,
    animations: Vec<Animation>,
    glbin_path: Option<PathBuf>,
    glbin_mirrored: bool,
    sequence: Option<&'a LSQ>,
}

impl<'a> LMT<'a> {
    pub fn import(buf: &[u8], scale: f32, fps: f32) -> Result<Self, Box<dyn std::error::Error>> {
        // No MAGIC for this file type unfortunately
        let spf = 1.0 / fps;

        let track_count_max = buf[0] as usize;
        let animation_count = buf[1] as usize;

        let header_end = 4 + animation_count * 8;
        let header_buf = &buf[4..header_end];

        let (header_list, []) = header_buf.as_chunks::<8>() else {
            panic!("header_buf length not a multiple of 8");
        };

        let mut animations: Vec<Animation> = Vec::with_capacity(animation_count);
        for header in header_list {
            animations.push(Animation {
                offset: LittleEndian::read_u32(&header[0..4]) as usize,
                duration: LittleEndian::read_u32(&header[4..8]) as f32 * spf,
                tracks: Vec::with_capacity(track_count_max),
            });
        }

        for animation in animations.iter_mut() {
            let animation_buf = &buf[animation.offset..];
            let mut track_offsets: Vec<u32> = vec![0; track_count_max];
            LittleEndian::read_u32_into(
                &animation_buf[..(track_count_max * 4)],
                &mut track_offsets,
            );

            for track_offset in track_offsets {
                if track_offset == 0 {
                    continue;
                }

                let track_buf = &animation_buf[track_offset as usize..];

                let node = LittleEndian::read_u16(&track_buf[0..2]) as u32;
                let frame_count = LittleEndian::read_u16(&track_buf[2..4]) as usize;

                let frame_buf_end = frame_count * LMT_FRAME_SIZE + 4;
                let (frame_list, []) = track_buf[4..frame_buf_end].as_chunks::<LMT_FRAME_SIZE>()
                else {
                    panic!("frame_list length not a multiple of LMT_FRAME_SIZE");
                };

                let mut time_min = f32::INFINITY;
                let mut time_max = f32::NEG_INFINITY;

                let mut frames: Vec<Frame> = Vec::with_capacity(frame_count);
                for frame in frame_list {
                    // The frame index is always sequential
                    let _index = LittleEndian::read_u16(&frame[0..2]);

                    let time = LittleEndian::read_u16(&frame[2..4]) as f32 * spf;

                    assert!(time <= animation.duration);

                    time_min = time_min.min(time);
                    time_max = time_max.max(time);

                    let rot = Vec3::new(
                        LittleEndian::read_f32(&frame[4..8]),
                        LittleEndian::read_f32(&frame[8..12]),
                        LittleEndian::read_f32(&frame[12..16]),
                    );

                    let pos = Vec3::new(
                        LittleEndian::read_f32(&frame[16..20]),
                        LittleEndian::read_f32(&frame[20..24]),
                        LittleEndian::read_f32(&frame[24..28]),
                    ) * scale;

                    frames.push(Frame { time, pos, rot })
                }

                animation.tracks.push(Track {
                    node,
                    frames,
                    time_min,
                    time_max,
                });
            }
        }

        return Ok(LMT {
            track_count_max,
            animations,
            glbin_path: None,
            glbin_mirrored: false,
            sequence: None,
        });
    }

    pub fn get_node_count(&self) -> usize {
        self.track_count_max
    }

    pub fn size(&self) -> usize {
        self.animations
            .iter()
            .fold(0, |acc, animation| acc + animation.size())
    }

    pub fn write_to_glbin(
        &mut self,
        path: &Path,
        include_mirror: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        for animation in &self.animations {
            for track in &animation.tracks {
                track.write(false, &mut writer)?;
            }
        }

        if include_mirror {
            for animation in &self.animations {
                for track in &animation.tracks {
                    track.write(true, &mut writer)?;
                }
            }
        }

        self.glbin_path = Some(path.to_path_buf());
        self.glbin_mirrored = include_mirror;

        return Ok(());
    }

    pub fn set_sequence(&mut self, lsq: &'a LSQ) -> Result<(), Box<dyn std::error::Error>> {
        if lsq.animation_count != self.animations.len() {
            return Err("AnimationCountIncorrect".into());
        }

        self.sequence = Some(lsq);

        return Ok(());
    }

    pub fn path_to_glbin(&self, path: &Path) -> Option<PathBuf> {
        let mut gltf_path = PathBuf::from(path);
        gltf_path.set_file_name("");

        return diff_paths(self.glbin_path.as_ref()?, gltf_path);
    }

    pub fn write_to_gltf(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let glbin_rel_path = if let Some(p) = self.path_to_glbin(path) {
            p
        } else {
            return Err("Cannot get relative path to glbin".into());
        };

        let mut root = Root::default();

        root.asset = Asset {
            copyright: None,
            extensions: None,
            generator: Some(String::from("bbtools")),
            min_version: None,
            extras: Void::default(),
            version: String::from("2.0"),
        };

        self.append_to_gltf(&mut root, &glbin_rel_path, None)?;

        let file = File::create(path)?;
        root.to_writer_pretty(file)?;

        return Ok(());
    }

    pub fn append_to_gltf(
        &self,
        root: &mut Root,
        gltf_path: &Path,
        mirror_nodes: Option<&[u8]>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if mirror_nodes.is_some() && !self.glbin_mirrored {
            return Err("Mirror nodes specified, but glbin is missing mirrored data".into());
        }

        let glbin_rel_path = if let Some(p) = self.path_to_glbin(gltf_path) {
            p
        } else {
            return Err("Cannot get relative path to animation glbin".into());
        };

        let glbin_rel_path_str = glbin_rel_path.to_string_lossy().into_owned();

        let keyframes_size = self.size() * (if self.glbin_mirrored { 2 } else { 1 });

        // Buffer for the LMT data
        let buf_idx = root.push(Buffer {
            name: Some(String::from("Keyframes")),
            uri: Some(glbin_rel_path_str),
            byte_length: USize64::from(keyframes_size),
            extensions: None,
            extras: Void::default(),
        });

        self.append_to_gltf_mirror(root, buf_idx, None)?;
        if mirror_nodes.is_some() {
            self.append_to_gltf_mirror(root, buf_idx, mirror_nodes)?;
        }

        if let Some(sequence) = self.sequence {
            sequence.append_to_gltf(root, gltf_path, mirror_nodes.is_some())?;
        }

        return Ok(());
    }

    fn append_to_gltf_mirror(
        &self,
        root: &mut Root,
        buf_idx: Index<Buffer>,
        mirror_nodes: Option<&[u8]>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let is_mirrored = mirror_nodes.is_some();

        let keyframes_size = self.size();
        let view_offset: Option<USize64> = if is_mirrored {
            Some(USize64::from(keyframes_size))
        } else {
            None
        };

        let view_name = if is_mirrored {
            "Keyframes Mirrored"
        } else {
            "Keyframes"
        };

        let key_view_idx = root.push(View {
            name: Some(String::from(view_name)),
            buffer: buf_idx,
            byte_length: USize64::from(keyframes_size),
            byte_offset: view_offset,
            byte_stride: Some(Stride(LMT_FRAME_STRIDE)),
            extensions: None,
            extras: Extras::default(),
            target: None,
        });

        let mut track_offset: u64 = 0;
        for (i, animation) in self.animations.iter().enumerate() {
            let mut samplers: Vec<Sampler> = Vec::with_capacity(animation.tracks.len());
            let mut channels: Vec<Channel> = Vec::with_capacity(animation.tracks.len());

            for (j, track) in animation.tracks.iter().enumerate() {
                let frame_count = USize64::from(track.frames.len());

                /*** Accessors ***/
                let time_acc_idx = root.push(Accessor {
                    name: Some(format!("Animation {}, Track {} - Time", i, j)),
                    buffer_view: Some(key_view_idx),
                    byte_offset: Some(USize64::from(track_offset)),
                    count: frame_count,
                    component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                    extensions: None,
                    extras: Extras::default(),
                    type_: Checked::Valid(Type::Scalar),
                    min: Some(json!([track.time_min])),
                    max: Some(json!([track.time_max])),
                    normalized: false,
                    sparse: None,
                });

                let pos_acc_idx = root.push(Accessor {
                    name: Some(format!("Animation {}, Track {} - Position", i, j)),
                    buffer_view: Some(key_view_idx),
                    byte_offset: Some(USize64::from(track_offset + 4)),
                    count: frame_count,
                    component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                    extensions: None,
                    extras: Extras::default(),
                    type_: Checked::Valid(Type::Vec3),
                    min: None,
                    max: None,
                    normalized: false,
                    sparse: None,
                });

                let rot_acc_idx = root.push(Accessor {
                    name: Some(format!("Animation {}, Track {} - Rotation", i, j)),
                    buffer_view: Some(key_view_idx),
                    byte_offset: Some(USize64::from(track_offset + 16)),
                    count: frame_count,
                    component_type: Checked::Valid(GenericComponentType(ComponentType::F32)),
                    extensions: None,
                    extras: Extras::default(),
                    type_: Checked::Valid(Type::Vec4),
                    min: None,
                    max: None,
                    normalized: false,
                    sparse: None,
                });

                /*** Samplers ***/
                let pos_smp_idx = Index::<Sampler>::new(samplers.len() as u32);
                samplers.push(Sampler {
                    extensions: None,
                    extras: Extras::default(),
                    input: time_acc_idx,
                    interpolation: Checked::Valid(Interpolation::Linear),
                    output: pos_acc_idx,
                });

                let rot_smp_idx = Index::<Sampler>::new(samplers.len() as u32);
                samplers.push(Sampler {
                    extensions: None,
                    extras: Extras::default(),
                    input: time_acc_idx,
                    interpolation: Checked::Valid(Interpolation::Linear),
                    output: rot_acc_idx,
                });

                let node_idx = Index::<Node>::new(if let Some(mn) = mirror_nodes {
                    mn[track.node as usize] as u32
                } else {
                    track.node
                });

                /*** Channels ***/
                channels.push(Channel {
                    sampler: pos_smp_idx,
                    target: animation::Target {
                        node: node_idx,
                        path: Checked::Valid(Property::Translation),
                        extensions: None,
                        extras: Extras::default(),
                    },
                    extensions: None,
                    extras: Extras::default(),
                });

                channels.push(Channel {
                    sampler: rot_smp_idx,
                    target: animation::Target {
                        node: node_idx,
                        path: Checked::Valid(Property::Rotation),
                        extensions: None,
                        extras: Extras::default(),
                    },
                    extensions: None,
                    extras: Extras::default(),
                });

                track_offset += track.size() as u64;
            }

            let mirror_str = if is_mirrored { "M" } else { "" };

            let mut animation_extension = Map::new();
            animation_extension.insert(String::from("bbtools"), json!({"mirrored": is_mirrored}));

            let _animation_idx = root.push(GltfAnimation {
                name: Some(format!("Anim_{}{}", i, mirror_str)),
                extensions: Some(AnimationExtension {
                    others: animation_extension,
                }),
                extras: Extras::default(),
                channels: channels,
                samplers: samplers,
            });
        }

        assert!(track_offset as usize == keyframes_size);

        return Ok(());
    }
}

impl fmt::Display for LMT<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Animation Count {:02}, Track Count Max {:02}",
            self.animations.len(),
            self.track_count_max
        )?;

        for (i, animation) in self.animations.iter().enumerate() {
            writeln!(
                f,
                "  Animation {:02}, Track Count {:02}, Duration {}",
                i,
                animation.tracks.len(),
                animation.duration
            )?;
        }

        return Ok(());
    }
}
