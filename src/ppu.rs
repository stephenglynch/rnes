use std::cell::{Cell, RefCell};
use std::rc::Rc;
use bitflags::bitflags;
use crate::clock::{Clock, CycleDelay};
use crate::renderer::FrameBuffer;
use crate::mapper::Mapper;
use palette::{Rgb, PaletteRam, Colour};
use oam::Oam;

mod palette;
mod oam;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

// Awaits 1 ppu cycle
macro_rules! cycles {
    ($ppu:expr, $n:expr) => {
        CycleDelay::new($ppu.clock.clone(), $n, false).await
    }
}

macro_rules! catchup {
    ($ppu:expr, $n:expr) => {
        CycleDelay::new($ppu.clock.clone(), $n, true).await
    }
}

macro_rules! frame_done {
    ($ppu:expr) => {
        CycleDelay::frame_done($ppu.clock.clone(), true).await
    }
}

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct PpuCtrl: u8 {
        const NAME_TABLE_0 = (1 << 0);
        const NAME_TABLE_1 = (1 << 1);
        const VRAM_INCR = (1 << 2);
        const SPRITE_TILE_SEL = (1 << 3);
        const BACKGROUND_SEL = (1 << 4);
        const SPRITE_HEIGHT = (1 << 5);
        const PPU_MASTER_SLAVE = (1 << 6);
        const NMI_EN = (1 << 7);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct PpuMask: u8 {
        const GREY_SCALE = (1 << 0);
        const SHOW_BACKGROUND = (1 << 1);
        const SHOW_SPRITES = (1 << 2);
        const RENDER_BACKGROUND = (1 << 3);
        const RENDER_SPRITES = (1 << 4);
        const EMPHASIZE_RED = (1 << 5);
        const EMPHASIZE_GREEN = (1 << 6);
        const EMPHASIZE_BLUE = (1 << 7);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct PpuStatus: u8 {
        const SPRITE_OVERFLOW = (1 << 5);
        const SPRITE_0_HIT = (1 << 6);
        const VBLANK = (1 << 7);
    }
}

pub struct RgbFrame([Rgb; WIDTH * HEIGHT]);

#[derive(Clone, Copy)]
struct VramAddr(u16);

pub struct Ppu {
    // CPU regs
    ppu_ctrl: Cell<PpuCtrl>,
    ppu_mask: Cell<PpuMask>,
    ppu_status: Cell<PpuStatus>,
    oam_addr: Cell<u8>,

    // Internal registers
    write_toggle: Cell<bool>,
    t_reg: Cell<VramAddr>,
    v_reg: Cell<VramAddr>,
    x_fine_reg: Cell<u8>,
    ppudata_read_buffer: Cell<u8>,

    // Other resources
    clock: Rc<RefCell<Clock>>,
    mapper: Rc<RefCell<dyn Mapper>>,
    palette_ram: Rc<RefCell<PaletteRam>>,
    oam: RefCell<Oam>,
    frame_buffer: FrameBuffer
}

impl RgbFrame {
    pub fn new() -> Self {
        RgbFrame([Rgb::new(); WIDTH * HEIGHT])
    }

    pub fn write_chunk(&mut self, line: usize, tile_x: usize, chunk: [Rgb; 8]) {
        let start = line * WIDTH + tile_x * 8;
        let end = start + 8;
        self.0[start..end].copy_from_slice(&chunk[0..8]);
    }

    pub fn to_rgb_frame(&self) -> Vec<u8> {
        self.0.iter().flat_map(|rgb| {
            [rgb.0, rgb.1, rgb.2, 0xff]
        }).collect()
    }
}

impl VramAddr {
    fn set_x(self, x_val: u16) -> Self {
        let mut v = self.0;
        v &= !0b00000100_00011111; // Clear x bits
        v |= (x_val & 0b11111000) >> 3; // Set x course bits
        v |= (x_val & 0b00000001_00000000) << 2; // Set scroll bit 8
        VramAddr(v)
    }

    fn set_y(self, y_val: u16) -> Self {
        let mut v = self.0;
        v &= !0b01111011_11100000; // Clear y bits
        v |= (y_val & 0b00000111) << 12; //  Set y fine bits
        let nt_bit = (y_val / 240) & 0x1; // If we are larger than 240 then we use nametable bit
        let y_val = y_val % 240; // We wrap on 240
        v |= (y_val & 0b11111000) << 2; // Set y course bits
        v |= nt_bit << 11; // Set scroll bit 8
        VramAddr(v)
    }

    fn x_course_increment(self) -> Self {
        let v = self.0;
        let mut x_course = (v & 0b11111) | ((v & 0b100_00000000) >> 5);
        x_course += 1;
        let x = x_course << 3;
        self.set_x(x)
    }

    // Based off psuedocode on nesdev
    fn y_increment(self) -> Self {
        let mut v = self.0;
        if (v & 0x7000) != 0x7000 {             // if fine Y < 7
            v += 0x1000;                        // increment fine Y
        } else {
            v &= !0x7000;                       // fine Y = 0
            let mut y = (v & 0x03E0) >> 5; // let y = coarse Y
            if y >= 29 {
                y = 0;                          // coarse Y = 0
                v ^= 0x0800;                    // switch vertical nametable
            } else if y == 31 {
                y = 0;                          // coarse Y = 0, nametable not switched
            } else {
                y += 1;                         // increment coarse Y
            }
            v = (v & !0x03E0) | (y << 5)        // put coarse Y back into v
        }
        Self(v)
    }

    fn set_x_raw(&self, other: Self) -> Self {
        let mask = 0b00000100_00011111;
        let mut new_v = self.0 & !mask;
        let other = other.0;
        new_v |= other & mask;
        VramAddr(new_v)
    }

    fn set_y_raw(&self, other: Self) -> Self {
        let mask = 0b01111011_11100000;
        let mut new_v = self.0 & !mask;
        let other = other.0;
        new_v |= other & mask;
        VramAddr(new_v)
    }
}

impl Ppu {
    pub fn new(clock: Rc<RefCell<Clock>>, mapper: Rc<RefCell<dyn Mapper>>, frame_buffer: FrameBuffer) -> Self {
        let palette_ram = Rc::new(RefCell::new(PaletteRam::new()));
        Self {
            //TODO: Confirm the intialisation values agianst power-on values
            clock: clock,
            ppu_ctrl: Cell::new(PpuCtrl::from_bits_retain(0)),
            ppu_mask: Cell::new(PpuMask::from_bits_retain(0)),
            ppu_status: Cell::new(PpuStatus::from_bits_retain(0)),
            oam_addr: Cell::new(0),
            t_reg: Cell::new(VramAddr(0)),
            v_reg: Cell::new(VramAddr(0)),
            x_fine_reg: Cell::new(0),
            write_toggle: Cell::new(false),
            ppudata_read_buffer: Cell::new(0),
            oam: RefCell::new(Oam::new(mapper.clone(), palette_ram.clone())),
            mapper: mapper,
            palette_ram: palette_ram,
            frame_buffer: frame_buffer
        }
    }

    fn apply_fine_x_scroll(&self, full_chunk: &[Colour; 16]) -> [Colour; 8] {
        let fine_x = self.x_fine_reg.get() as usize;
        let mut output_chunk = [Colour::new(); 8];
        output_chunk.copy_from_slice(&full_chunk[fine_x..(fine_x + 8)]);
        output_chunk
    }

    fn v_hor_inc(&self) {
        self.v_reg.set(self.v_reg.get().x_course_increment());
    }

    fn v_ver_inc(&self) {
        self.v_reg.set(self.v_reg.get().y_increment());
    }

    fn reset_v_hor(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_x_raw(t));
    }

    fn reset_v_ver(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_y_raw(t));
    }

    fn is_rendering(&self) -> bool {
        self.ppu_mask.get().intersects(PpuMask::RENDER_BACKGROUND | PpuMask::RENDER_SPRITES)
    }

    pub async fn run(&self) {
        // Not correctly updating v
        let mut chunk;
        let mut full_chunk = [Colour::new(); 16];
        let mut current_sprite_line = [[Colour::new(); 8]; 32];
        let mut next_sprite_line = [[Colour::new(); 8]; 32];
        let mut frame = RgbFrame::new();
        let mut odd_frame = true;
        loop {
            // Render lines (-1, 0)
            for line in -1..240i32 {
                // Render line (line, 0)
                // Check if skip dot (0, 0)
                if odd_frame || line != 0 {
                    cycles!(self, 1);
                }

                // (-1, 1)
                if line == -1 {
                    self.ppu_status.set(self.ppu_status.get() & !PpuStatus::VBLANK);
                    self.ppu_status.set(self.ppu_status.get() & !PpuStatus::SPRITE_0_HIT);
                }

                // (line, 1)
                self.oam.borrow_mut().clear_secondary_oam();
                if line != -1 {
                    self.oam.borrow_mut().prepare_oam(line as usize);
                }
                for tile in 0..32 {
                    if self.is_rendering() {
                        // Render chunk unless it's the pre-render line
                        if line != -1 && tile != 31 {
                            let drawn_bg = self.apply_fine_x_scroll(&full_chunk);
                            next_sprite_line[tile] = self.draw_oam(tile as usize);
                            let drawn_chunk = self.combine_drawn_layers(drawn_bg, current_sprite_line[tile]);
                            frame.write_chunk(line as usize, tile as usize, drawn_chunk);
                        }
                        // Get next chunk
                        let last_chunk = &full_chunk[8..16].to_vec();
                        full_chunk[0..8].copy_from_slice(&last_chunk);
                        chunk = self.render_line_chunk();
                        full_chunk[8..16].copy_from_slice(&chunk[0..8]);
                        cycles!(self, 7);

                        // (line, 256)
                        if tile == 31 {
                            self.v_ver_inc();
                        }
                        self.v_hor_inc();
                        cycles!(self, 1);
                    } else {
                        cycles!(self, 8);
                    }
                    current_sprite_line = next_sprite_line;
                }

                // (line, 257)
                if self.is_rendering() {
                    self.reset_v_hor();
                }
                cycles!(self, 23);

                // (line == -1, 280 - 304) We reset the v addr constantly here
                if line == -1 && self.is_rendering() {
                    for _ in 0..25 {
                        self.reset_v_ver();
                        cycles!(self, 1);
                    }
                } else {
                    cycles!(self, 25);
                }

                // (line, 305)
                cycles!(self, 16);

                // (line, 321) Chunks for next line

                // Get next chunk
                if self.is_rendering() {
                    full_chunk[0..8].copy_from_slice(&self.render_line_chunk());
                    cycles!(self, 7);
                    self.v_hor_inc();
                    cycles!(self, 1);
                    full_chunk[8..16].copy_from_slice(&self.render_line_chunk());
                    cycles!(self, 7);
                    self.v_hor_inc();
                    cycles!(self, 1);
                } else {
                    cycles!(self, 16);
                }

                // (line, 337) Nothing really happens for the rest of the line
                cycles!(self, 4);
            }

            // This marks end of rendering and start of V-blank
            // Post-render line (240, 0)
            cycles!(self, 340 + 1);
            // Post-render line (241, 1)
            self.ppu_status.set(self.ppu_status.get() | PpuStatus::VBLANK);
            catchup!(self, 340 + 341 * 19);

            self.write_to_frame_buffer(&frame);
            frame_done!(self);

            odd_frame = !odd_frame;
        }
    }

    fn write_to_frame_buffer(&self, frame: &RgbFrame) {
        let mut frame_buffer = self.frame_buffer.lock().unwrap();
        frame_buffer.copy_from_slice(&frame.to_rgb_frame());
    }

    fn mmu_load(&self, addr: u16) -> u8 {
        let addr = addr & 0x3fff;
        match addr {
            0x0000..0x3f00 => self.mapper.borrow_mut().ppu_get(addr as usize),
            0x3f00..0x4000 => self.palette_ram.borrow_mut().get((addr & 0x001f) as usize),
            0x4000.. => 0
        }
    }

    fn mmu_store(&self, addr: u16, val: u8) {
        let addr = addr & 0x3fff;
        match addr {
            0x0000..0x3f00 => self.mapper.borrow_mut().ppu_set(addr as usize, val),
            0x3f00..0x4000 => self.palette_ram.borrow_mut().set((addr & 0x001f) as usize, val),
            0x4000.. => ()
        }
    }

    fn palette_lookup(&self, course_x: u16, course_y: u16, nt_sel: u16) -> usize {
        let at = self.mmu_load((course_x >> 2) | ((course_y >> 2) << 3) | (nt_sel << 10) | 0x23c0);
        (match (course_x & 0b10 != 0, course_y & 0b10 != 0) {
            (false, false) => at & 0b00000011,
            (false, true)  => (at & 0b00110000) >> 4,
            (true, false)  => (at & 0b00001100) >> 2,
            (true, true)   => (at & 0b11000000) >> 6,
        }) as usize
    }

    fn render_line_chunk(&self) -> [Colour; 8] {
        let v = self.v_reg.get().0;
        let mut colour = [Colour::new(); 8];
        let course_x = (v) & 0b11111;
        let course_y = (v >> 5) & 0b11111;
        let nt_sel = (v >> 10) & 0b11;
        let fine_y = (v >> 12) & 0b111;

        // Fetch nametable byte
        let nt = self.mmu_load((v & 0x0fff) | 0x2000) as u16;

        // Fetch palette from attribute byte
        let palette = self.palette_lookup(course_x, course_y, nt_sel);

        // Fetch pattern bytes
        let half = self.ppu_ctrl.get().contains(PpuCtrl::BACKGROUND_SEL) as u16;
        let pattern_0 = self.mmu_load((half << 12) | (nt << 4) | 0x0 | (fine_y));
        let pattern_1 = self.mmu_load((half << 12) | (nt << 4) | 0x8 | (fine_y));

        // Fetch pixel colour
        for bit in 0..8 {
            let col_sel_0 = pattern_0 & (1 << bit) != 0;
            let col_sel_1 = pattern_1 & (1 << bit) != 0;
            let bit_colour = match (col_sel_1, col_sel_0) {
                (false, false) => self.palette_ram.borrow().rgb_lookup(palette, 0b00, false),
                (false, true)  => self.palette_ram.borrow().rgb_lookup(palette, 0b01, false),
                (true, false)  => self.palette_ram.borrow().rgb_lookup(palette, 0b10, false),
                (true, true)   => self.palette_ram.borrow().rgb_lookup(palette, 0b11, false)
            };

            colour[bit] = bit_colour;
        }

        colour.reverse();
        colour
    }

    #[inline(never)]
    fn combine_drawn_layers(&self, background: [Colour; 8], sprite: [Colour; 8]) -> [Rgb; 8] {
        let backdrop = self.palette_ram.borrow().backdrop_colour();
        let mut drawn_chunk = [Rgb::new(); 8];
        background.iter().zip(sprite.iter()).enumerate().for_each(|(i, pair)| {
            let (&a, &b) = pair;
            let pair = (a, b);
            drawn_chunk[i] = match pair {
                (Colour::Transparent, Colour::Transparent) => backdrop,
                (Colour::Rgb(background_rgb), Colour::Sprite(sprite_rgb, sprite0, priority)) => {
                    if sprite0 {
                        self.ppu_status.set(self.ppu_status.get() | PpuStatus::SPRITE_0_HIT);
                    }
                    if !priority {
                        sprite_rgb
                    } else {
                        background_rgb
                    }
                },
                (Colour::Rgb(_), Colour::Rgb(sprite_rgb)) => sprite_rgb,
                (Colour::Transparent, Colour::Sprite(sprite_rgb, _, _)) => sprite_rgb,
                (Colour::Transparent, Colour::Rgb(sprite_rgb)) => sprite_rgb,
                (Colour::Rgb(bg_rgb), Colour::Transparent) => bg_rgb,
                _ => panic!("No match for {:?}", pair)
            }
        });
        drawn_chunk
    }

    fn v_addr_increment(&self) {
        let vram_incr_flag = self.ppu_ctrl.get().contains(PpuCtrl::VRAM_INCR);
        self.v_reg.set(VramAddr(self.v_reg.get().0.wrapping_add(
            if vram_incr_flag {
                32
            } else {
                1
            }))
        );
    }

    fn read_status(&self) -> u8 {
        self.write_toggle.set(false);
        self.ppu_status.get().bits()
    }

    pub fn set_reg(&self, addr: usize, val: u8) {
        let write_toggle = self.write_toggle.get();
        match addr {
            // PPUCTRL
            0 => {
                self.ppu_ctrl.set(PpuCtrl::from_bits_retain(val));
                let mut t = self.t_reg.get().0;
                t &= !(0b0001100_00000000); // Clear nametable bits
                t |= (val as u16 & 0b00000011) << 10; // Set the new nametable bits
                self.t_reg.set(VramAddr(t));
            },
            // PPUMASK
            1 => self.ppu_mask.set(PpuMask::from_bits_retain(val)),
            // PPUSTATUS
            2 => (),
            // OAMADDR
            3 => self.oam_addr.set(val),
            // OAMDATA
            4 => self.write_oam(val),
            // PPUSCROLL
            5 => {
                let t = self.t_reg.get();
                self.t_reg.set(if !write_toggle {
                    let t = t.set_x(val as u16);
                    let x_fine = 0b0111 & val;
                    self.x_fine_reg.set(x_fine);
                    t
                } else {
                    t.set_y(val as u16)
                });
                self.write_toggle.set(!write_toggle);
            },
            // PPUADDR
            6 => {
                let mut t = self.t_reg.get().0;
                if !write_toggle {
                    t &= !0xff00; // Clear MSB byte
                    t |= ((val & 0b00111111) as u16) << 8; // Set MSB bytes but clear bits above bit 13
                    self.t_reg.set(VramAddr(t));
                } else {
                    t &= !0x00ff;
                    t |= val as u16;
                    self.t_reg.set(VramAddr(t));
                    self.v_reg.set(VramAddr(t));
                };
                self.write_toggle.set(!write_toggle);
            },
            // PPUDATA
            7 => {
                self.mmu_store(self.v_reg.get().0, val);
                self.v_addr_increment();
            },
            _ => unreachable!()
        }
    }

    pub fn get_reg(&self, addr: usize) -> u8 {
        match addr {
            // PPUCTRL
            0 => 0,
            // PPUMASK
            1 => 0,
            // PPUSTATUS
            2 => self.read_status(),
            // OAMADDR
            3 => 0,
            // OAMDATA
            4 => self.read_oam(),
            // PPUSCROLL
            5 => 0,
            // PPUADDR
            6 => 0,
            // PPUDATA
            7 => {
                let val = self.mmu_load(self.v_reg.get().0);
                let read_buffer_val = self.ppudata_read_buffer.get();
                self.ppudata_read_buffer.set(val);
                self.v_addr_increment();
                read_buffer_val
            },
            _ => unreachable!()
        }
    }

    pub fn nmi_request(&self) -> bool {
        let vblank = self.ppu_status.get().contains(PpuStatus::VBLANK);
        let nmi_enable = self.ppu_ctrl.get().contains(PpuCtrl::NMI_EN);
        vblank && nmi_enable
    }

    fn read_oam(&self) -> u8 {
        self.oam.borrow().read(self.oam_addr.get() as usize)
    }

    pub fn write_oam(&self, val: u8) {
        let oam_addr = self.oam_addr.get();
        self.oam.borrow_mut().write(oam_addr as usize, val);
        self.oam_addr.set(oam_addr.wrapping_add(1));
    }

    fn draw_oam(&self, x_course: usize) -> [Colour; 8] {
        let tile_sel = self.ppu_ctrl.get().contains(PpuCtrl::SPRITE_TILE_SEL);
        self.oam.borrow().draw_chunk(tile_sel, x_course)
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_vram_x_incr_1() {
        let vram = VramAddr(0x1062);
        let vram = vram.x_course_increment();
        assert_eq!(vram.0, 0x1063);
    }

    #[test]
    fn test_vram_x_incr_2() {
        let vram = VramAddr(0x05ff);
        let vram = vram.x_course_increment();
        assert_eq!(vram.0, 0x01e0);
    }

    #[test]
    fn test_vram_y_incr_1() {
        let vram = VramAddr(0x73a2);
        let vram = vram.y_increment();
        assert_eq!(vram.0, 0x0802);
    }

    #[test]
    fn test_vram_y_incr_2() {
        let vram = VramAddr(0x7002);
        let vram = vram.y_increment();
        assert_eq!(vram.0, 0x0022);
    }

    #[test]
    fn test_vram_y_incr_3() {
        let vram = VramAddr(0x0802);
        let vram = vram.y_increment();
        assert_eq!(vram.0, 0x1802);
    }
}