use std::cmp;
use std::io::{Seek, SeekFrom};

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

use crate::bbtools::xbe::XBE;

pub struct X86Context {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
    esi: u32,
    edi: u32,
    esp: i32,
    ebp: u32,
    stack: [u8; 0xFFFF],
    esp_min: i32,
}

impl X86Context {
    pub fn new(stack_size: usize) -> Self {
        let esp = -(stack_size as i32);
        return X86Context {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
            esi: 0,
            edi: 0,
            esp,
            ebp: 0,
            stack: [0; _],
            esp_min: esp,
        };
    }

    pub fn execute_instruction(&mut self, xbe: &mut XBE) -> Result<usize, std::io::Error> {
        let mut bytes_read: usize = 0;

        let opcode = xbe.reader.read_u8()?;
        bytes_read += 1;

        match opcode {
            // Subtract
            0x81 => {
                let dest_type = xbe.reader.read_u8()?;
                assert!(dest_type == 0xEC); // Subtract forom stack pointer
                bytes_read += 1;

                let offset = xbe.reader.read_i32::<LittleEndian>()?;
                bytes_read += 4;

                self.adjust_stack_pointer(-offset);
            }
            // OR
            0x83 => {
                let source_register = xbe.reader.read_u8()?;
                match source_register {
                    0xC8 => {
                        self.eax |= (xbe.reader.read_i8()? as i32) as u32; // Sign-extend then cast as u32
                    }
                    _ => {
                        println!("SR {:02X}", source_register);
                        todo!("unknown 0x83 source_register")
                    }
                }
            }
            // Push register to stack
            0x50 | 0x53 | 0x55 | 0x56 | 0x57 => {
                self.adjust_stack_pointer(-4);
            }
            // Pop register from stack
            0x5E | 0x5F => {
                self.adjust_stack_pointer(4);
            }
            // Load register
            0xB8 => {
                self.eax = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xB9 => {
                self.ecx = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xBA => {
                self.edx = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xBB => {
                self.ebx = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xBE => {
                self.esi = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xBD => {
                self.ebp = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            0xBF => {
                self.edi = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;
            }
            // 16-bit move
            0x66 => {
                let source_type = xbe.reader.read_u8()?;
                let source_register = xbe.reader.read_u8()?;
                assert!(xbe.reader.read_u8()? == 0x24);
                bytes_read += 3;

                let offset = if source_register & 0x80 == 0x80 {
                    bytes_read += 4;
                    xbe.reader.read_i32::<LittleEndian>()?
                } else {
                    bytes_read += 1;
                    xbe.reader.read_u8()? as i32
                };

                let value = match source_type {
                    // move register
                    0x89 => match source_register {
                        0x84 | 0x44 => self.eax as u16,
                        0xAC => self.ebp as u16,
                        0xBC => self.edi as u16,
                        0x94 => self.edx as u16,
                        0x9C => self.ebx as u16,
                        _ => {
                            println!("SR {:02X}", source_register);
                            todo!("unknown 0x66 0x89 source_register");
                        }
                    },
                    // move constant
                    0xC7 => {
                        bytes_read += 2;
                        xbe.reader.read_u16::<LittleEndian>()?
                    }
                    _ => todo!("unknown 0x66 source_type"),
                };

                let stack_offset = self.get_stack_offset(offset);
                LittleEndian::write_u16(&mut stack_offset[..2], value);
            }
            // 32-bit move register to stack
            0x89 => {
                let source_register = xbe.reader.read_u8()?;
                assert!(xbe.reader.read_u8()? == 0x24);
                bytes_read += 2;

                let offset = if source_register & 0x80 == 0x80 {
                    bytes_read += 4;
                    xbe.reader.read_i32::<LittleEndian>()?
                } else {
                    bytes_read += 1;
                    xbe.reader.read_u8()? as i32
                };

                let value = match source_register {
                    0x84 | 0x44 => self.eax,
                    0x5C => self.ebx,
                    0x8C | 0x4C => self.ecx,
                    0x94 | 0x54 => self.edx,
                    0xB4 | 0x74 => self.esi,
                    0xBC | 0x7C => self.edi,
                    _ => {
                        println!("SR {:02X}", source_register);
                        todo!("unknown 0x89 source_register");
                    }
                };

                //println!("Write U32 to offset: {:04X}, Stack size: {:04X}", offset, self.stack.len());
                let stack_offset = self.get_stack_offset(offset);
                LittleEndian::write_u32(&mut stack_offset[..4], value);
            }
            // 32-bit move constant to stack
            0xC7 => {
                let source_register = xbe.reader.read_u8()?;
                assert!(xbe.reader.read_u8()? == 0x24);
                bytes_read += 2;

                let offset = if source_register & 0x80 == 0x80 {
                    bytes_read += 4;
                    xbe.reader.read_i32::<LittleEndian>()?
                } else {
                    bytes_read += 1;
                    xbe.reader.read_u8()? as i32
                };

                let value = xbe.reader.read_u32::<LittleEndian>()?;
                bytes_read += 4;

                let stack_offset = self.get_stack_offset(offset);
                LittleEndian::write_u32(&mut stack_offset[..4], value);
            }
            // Move from memory to register
            0xA1 => {
                // Read pointer and remember the current position
                let pointer = xbe.reader.read_u32::<LittleEndian>()?;
                let position = xbe.reader.stream_position()?;

                // Seek to the pointer address and read the value into register
                self.eax = if xbe.seek_pointer_offset(pointer).is_err() {
                    0
                } else {
                    xbe.reader.read_u32::<LittleEndian>()?
                };

                // Return to the original position
                xbe.reader.seek(SeekFrom::Start(position))?;
            }
            // Test
            0xF6 => {
                let register_a = xbe.reader.read_u8()?;
                assert!(register_a == 0xC4);

                let _value = xbe.reader.read_u8()?;
            }
            _ => {
                println!("OP {:02X}", opcode);
                todo!("unknown opcode")
            }
        }

        return Ok(bytes_read);
    }

    pub fn adjust_stack_pointer(&mut self, offset: i32) {
        self.esp += offset as i32;
        self.esp_min = cmp::min(self.esp_min, self.esp);

        assert!((-self.esp) as usize <= self.stack.len());

        // Adjust stack
        let new_stack_size = (-self.esp) as usize;
        if new_stack_size > self.stack.len() {
            // self.stack.resize(new_stack_size, 0);
            /*
            // Create a new stack and place the contents of the old stack at the end
            let mut new_stack: Vec<u8> = Vec::with_capacity(new_stack_size);
            new_stack.resize(new_stack_size - self.stack.len(), 0);
            new_stack.append(&mut self.stack);
            self.stack = new_stack;
            */
        }
    }

    pub fn get_stack_offset(&mut self, offset: i32) -> &mut [u8] {
        let esp_offset = self.esp + offset;
        let stack_offset = esp_offset + self.stack.len() as i32;
        assert!(stack_offset >= 0);
        /*
        println!(
            "ESP: {}, OFFSET: {:08X}, ESP_OFFSET: {}, STACK_OFFSET: {}",
            self.esp, offset, esp_offset, stack_offset
        );
        */
        &mut self.stack[(stack_offset as usize)..]
    }

    pub fn get_stack(&self) -> &[u8] {
        let stack_start = self.esp_min + self.stack.len() as i32;
        assert!(stack_start >= 0);
        &self.stack[stack_start as usize..]
    }
}
