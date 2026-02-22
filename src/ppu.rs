// TODO: Support palettes

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use arrayvec::ArrayVec;
use bitflags::bitflags;
use crate::clock::{Clock, CycleDelay};
use crate::renderer::FrameBuffer;
use palette::{Rgb, PaletteRam, Colour};
use oam::Oam;

mod palette;
mod oam;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

// Awaits 1 ppu cycle
macro_rules! cycles {
    ($ppu:expr, $n:expr) => {
        CycleDelay::new($ppu.clock.clone(), $n).await
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

    // Loopy regs
    write_toggle: Cell<bool>,
    t_reg: Cell<VramAddr>,
    v_reg: Cell<VramAddr>,
    x_fine_reg: Cell<u8>,

    // Other resources
    clock: Rc<RefCell<Clock>>,
    chr_rom: Rc<RefCell<Vec<u8>>>,
    ram: RefCell<Vec<u8>>,
    palette_ram: Rc<RefCell<PaletteRam>>,
    oam: RefCell<Oam>,
    frame_buffer: FrameBuffer
}

enum Memory {
    ChrRom,
    Ram,
    PaletteRam,
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

    fn set_x_course(self, val: u16) -> Self {
        let mut v = self.0;
        v &= !0b00000000_00011111; // Clear x-scroll bits and nametable bits
        v |= (val & 0b11111000) >> 3; // Set course scroll bits (3-7)
        VramAddr(v)
    }

    fn set_x(self, val: u16) -> Self {
        let mut v = self.set_x_course(val).0;
        v |= (val & 0b00000001_00000000) << 2; // Set scroll bit 8
        VramAddr(v)
    }

    fn set_y_course(self, val: u16) -> Self {
        let val = val;
        let mut v = self.0;
        v &= !0b00000011_11100000; // Clear course y-scroll bits
        v |= (val & 0b11111000) << 2;
        VramAddr(v)
    }

    fn set_y_fine(self, val: u16) -> Self {
        let val = val;
        let mut v = self.0;
        v &= !0b01110000_00000000; // Clear fine y-scroll bits
        v |= (val & 0b00000111) << 12;
        VramAddr(v)
    }

    fn set_y(self, val: u16) -> Self {
        let v = self.set_y_course(val);
        let mut v = v.set_y_fine(val).0;
        v |= (val & 0b00000001_00000000) << 3; // Set scroll bit 8
        VramAddr(v)
    }

    fn to_x_course(&self) -> u16 {
        let mut v = 0;
        v |= (0b00000000_00011111 & self.0) << 3;
        v |= (0b00000100_00000000 & self.0) >> 2;
        v
    }

    fn to_y(&self) -> u16 {
        let mut y = 0;
        y |= (self.0 & 0b01110000_00000000) >> 12;
        y |= (self.0 & 0b00000011_11100000) >> 2;
        y |= (self.0 & 0b00001000_00000000) >> 3;
        y
    }

    fn add_x_course(self, val: u16) -> Self {
        let val = self.to_x_course() + val;
        self.set_x(val)
    }

    fn add_y(self, val: u16) -> Self {
        let val = self.to_y() + val;
        self.set_y(val)
    }

    fn set_x_scroll_with_addr(&self, other: Self) -> Self {
        let mut new_v = self.0 & !0b00001100_00011111;
        let other = other.0;
        new_v |= other & 0b00001100_00011111;
        VramAddr(new_v)
    }

    fn set_y_scroll_with_addr(&self, other: Self) -> Self {
        let mut new_v = self.0 & !0b01110011_11100000;
        let other = other.0;
        new_v |= other & 0b01110011_11100000;
        VramAddr(new_v)
    }
}

impl Ppu {
    pub fn new(clock: Rc<RefCell<Clock>>, chr_rom: Vec<u8>, frame_buffer: FrameBuffer) -> Self {
        let chr = Rc::new(RefCell::new(chr_rom));
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
            oam: RefCell::new(Oam::new(chr.clone(), palette_ram.clone())),
            chr_rom: chr,
            ram: RefCell::new(vec![0; 4096]),
            palette_ram: palette_ram,
            frame_buffer: frame_buffer
        }
    }

    fn apply_fine_x_scroll(&self, full_chunk: &[Colour; 16]) -> [Colour; 8] {
        let fine_x = self.x_fine_reg.get() as usize;
        let mut output_chunk = [Colour::new(); 8];
        output_chunk.copy_from_slice(&full_chunk[fine_x..(fine_x + 8)]);
        output_chunk.reverse();
        output_chunk
    }

    fn v_hor_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_x_course(8));
    }

    fn v_ver_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_y(1));
    }

    fn reset_v_hor(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_x_scroll_with_addr(t));
    }

    fn reset_v_ver(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_y_scroll_with_addr(t));
    }

    fn is_rendering(&self) -> bool {
        self.ppu_mask.get().intersects(PpuMask::RENDER_BACKGROUND | PpuMask::RENDER_SPRITES)
    }

    pub async fn run(&self) {
        // Not correctly updating v
        let mut chunk;
        let mut full_chunk = [Colour::new(); 16];
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
                    self.oam.borrow_mut().populate_secondary_oam(line as usize);
                }
                for tile in 0..32 {
                    if self.is_rendering() {
                        // Render chunk unless it's the pre-render line
                        if line != -1 && tile != 31 {
                            let drawn_bg = self.apply_fine_x_scroll(&full_chunk);
                            let drawn_sprite = self.draw_oam(line as usize, tile as usize);
                            let drawn_chunk = self.combine_drawn_layers(drawn_bg, drawn_sprite);
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
                }

                // (line, 257)
                if self.is_rendering() {
                    self.reset_v_hor();
                }
                cycles!(self, 23);

                // (line == -1, 280 - 304) We reset the v addr constantly here
                if line == -1 && self.is_rendering() {
                    for _ in 0..24 {
                        self.reset_v_ver();
                        cycles!(self, 1);
                    }
                } else {
                    cycles!(self, 24);
                }

                cycles!(self, 17);

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

                // (l, 256) Nothing really happens for the rest of the line
                cycles!(self, 84);
            }

            // This marks end of rendering and start of V-blank
            // Post-render line (240, 0)
            cycles!(self, 340 + 1);
            // Post-render line (241, 1)
            self.ppu_status.set(self.ppu_status.get() | PpuStatus::VBLANK);
            cycles!(self, 340 + 341 * 19);

            self.write_to_frame_buffer(&frame);

            odd_frame = !odd_frame;
        }
    }

    fn write_to_frame_buffer(&self, frame: &RgbFrame) {
        let mut frame_buffer = self.frame_buffer.lock().unwrap();
        frame_buffer.copy_from_slice(&frame.to_rgb_frame());
    }

    fn mmu_resolve(&self, addr: u16) -> (Memory, usize) {
        let (mem, loc) = match addr {
            0x0000..0x2000 => (Memory::ChrRom, addr),
            0x2000..0x3f00 => (Memory::Ram, addr & 0x0fff),
            0x3f00..0x3fff => (Memory::PaletteRam, addr & 0x001f),
                         _ => panic!("Unexpected vram access: 0x{:04x}", addr)
        };
        (mem, loc as usize)
    }

    fn mmu_load(&self, addr: u16) -> u8 {
        // println!("Loading from 0x{:04x}", addr);
        let (mem, addr) = self.mmu_resolve(addr);
        match mem {
            Memory::ChrRom => self.chr_rom.borrow()[addr],
            Memory::Ram => self.ram.borrow()[addr],
            Memory::PaletteRam => self.palette_ram.borrow().get(addr)
        }
    }

    fn mmu_store(&self, addr: u16, val: u8) {
        let (mem, addr) = self.mmu_resolve(addr);
        match mem {
            Memory::ChrRom => (), // Do nothing if writing into ROM
            Memory::Ram => self.ram.borrow_mut()[addr] = val,
            Memory::PaletteRam => self.palette_ram.borrow_mut().set(addr, val),
        }
    }

    fn palette_lookup(&self, course_x: u16, course_y: u16, nt_sel: u16) -> usize {
        let at = self.mmu_load((course_x >> 2) | ((course_y >> 2) << 3) | (nt_sel << 6) | 0x23c0);
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

        colour
    }

    fn combine_drawn_layers(&self, background: [Colour; 8], sprite: [Colour; 8]) -> [Rgb; 8] {
        let backdrop = self.palette_ram.borrow().backdrop_colour();
        background.iter().zip(sprite.iter()).map(|pair| {
            match pair {
                (Colour::Transparent, Colour::Transparent) => backdrop,
                (Colour::Rgb(_), Colour::Sprite0(sprite_rgb)) => {
                    self.ppu_status.set(self.ppu_status.get() | PpuStatus::SPRITE_0_HIT);
                    *sprite_rgb
                },
                (Colour::Rgb(_), Colour::Rgb(sprite_rgb)) => *sprite_rgb,
                (Colour::Transparent, Colour::Sprite0(sprite_rgb)) => *sprite_rgb,
                (Colour::Transparent, Colour::Rgb(sprite_rgb)) => *sprite_rgb,
                (Colour::Rgb(bg_rgb), Colour::Transparent) => *bg_rgb,
                _ => panic!("No match for {:?}", pair)
            }
        }).collect::<ArrayVec<Rgb, 8>>().into_inner().unwrap()
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
            0 => {
                self.ppu_ctrl.set(PpuCtrl::from_bits_retain(val));
                let mut t = self.t_reg.get().0;
                t &= !(0b0001100_00000000); // Clear nametable bits
                t |= (val as u16 & 0b00000011) << 10; // Set the new nametable bits
                self.t_reg.set(VramAddr(t));
            },
            1 => self.ppu_mask.set(PpuMask::from_bits_retain(val)),
            2 => (),
            3 => self.oam_addr.set(val),
            4 => self.write_oam(val),
            5 => {
                let t = self.t_reg.get();
                self.t_reg.set(if !write_toggle {
                    t.set_x_course(val as u16);
                    let x_fine = 0b0111 & val;
                    self.x_fine_reg.set(x_fine);
                    t
                } else {
                    t.set_y(val as u16)
                });
                self.write_toggle.set(!write_toggle);
            },
            6 => {
                let mut t = self.t_reg.get().0;
                if !write_toggle {
                    t &= !0xff00; // Clear MSB byte
                    t |= ((val & 0b00111111) as u16) << 8; // Set MSB bytes but clear bits above bit 13
                } else {
                    t &= !0x00ff;
                    t |= val as u16;
                };
                self.write_toggle.set(!write_toggle);
                let t = VramAddr(t);
                self.t_reg.set(t);
                self.v_reg.set(t);
            },
            7 => {
                self.mmu_store(self.v_reg.get().0, val);
                self.v_addr_increment();
            },
            _ => unreachable!()
        }
    }

    pub fn get_reg(&self, addr: usize) -> u8 {
        match addr {
            0 => 0,
            1 => 0,
            2 => self.read_status(),
            3 => 0,
            4 => self.read_oam(),
            5 => 0,
            6 => 0,
            7 => {
                let val = self.mmu_load(self.v_reg.get().0);
                self.v_addr_increment();
                val
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

    fn draw_oam(&self, y: usize, x_course: usize) -> [Colour; 8] {
        let tile_sel = self.ppu_ctrl.get().contains(PpuCtrl::SPRITE_TILE_SEL);
        self.oam.borrow().draw_chunk(tile_sel, y, x_course)
    }
}