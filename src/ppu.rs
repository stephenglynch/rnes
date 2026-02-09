use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use bitflags::bitflags;
use crate::clock::{Clock, CycleDelay};

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

#[derive(Clone, Copy)]
pub struct Rgb {
    pub red: u8,
    pub blue: u8,
    pub green: u8
}

impl Rgb {
    fn new() -> Self {
        Rgb {red: 0, blue: 0, green: 0}
    }
}


pub struct RgbFrame([Rgb; WIDTH * HEIGHT]);

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
            [rgb.red, rgb.green, rgb.blue, 0xff]
        }).collect()
    }
}

#[derive(Clone, Copy)]
enum PalletId {
    Transparent,
    Pallet1,
    Pallet2,
    Pallet3
}

#[derive(Clone, Copy)]
struct VramAddr(u16);

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
    chr_rom: RefCell<Vec<u8>>,
    ram: RefCell<Vec<u8>>,
    palette_ram: RefCell<Vec<u8>>,
    oam_data: RefCell<Vec<u8>>,
    frame_sender: mpsc::Sender<Vec<u8>>
}

enum Memory {
    ChrRom,
    Ram,
    PaletteRam,
}


impl Ppu {
    pub fn new(clock: Rc<RefCell<Clock>>, chr_rom: Vec<u8>, frame_sender: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            //TODO: To be more accurate these should initialise to random values
            clock: clock,
            ppu_ctrl: Cell::new(PpuCtrl::from_bits_retain(0)),
            ppu_mask: Cell::new(PpuMask::from_bits_retain(0)),
            ppu_status: Cell::new(PpuStatus::from_bits_retain(0)),
            oam_addr: Cell::new(0),
            t_reg: Cell::new(VramAddr(0)),
            v_reg: Cell::new(VramAddr(0)),
            x_fine_reg: Cell::new(0),
            write_toggle: Cell::new(false),
            oam_data: RefCell::new(Vec::new()), // TODO change this, just needed to compile
            chr_rom: RefCell::new(chr_rom),
            ram: RefCell::new(vec![0; 4096]),
            palette_ram: RefCell::new(vec![0; 32]),
            frame_sender: frame_sender
        }
    }

    fn apply_fine_x_scroll(&self, full_chunk: &[Rgb; 16]) -> [Rgb; 8] {
        let fine_x = self.x_fine_reg.get() as usize;
        let mut output_chunk = [Rgb::new(); 8];
        output_chunk.copy_from_slice(&full_chunk[fine_x..(fine_x + 8)]);
        output_chunk.reverse();
        output_chunk
    }

    fn v_hor_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_x_course(8));
        println!("inc hori(v) = {:04x}", self.v_reg.get().0);
    }

    fn v_ver_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_y(1));
        println!("inc vert(v) = {:04x}", self.v_reg.get().0);
    }

    fn reset_v_hor(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_x_scroll_with_addr(t));
        println!("reset hori(v) = {:04x}", self.v_reg.get().0);
    }

    fn reset_v_ver(&self) {
        let v = self.v_reg.get();
        let t = self.t_reg.get();
        self.v_reg.set(v.set_y_scroll_with_addr(t));
        println!("reset vert(v) = {:04x}", self.v_reg.get().0);
    }

    fn is_rendering(&self) -> bool {
        self.ppu_mask.get().intersects(PpuMask::RENDER_BACKGROUND | PpuMask::RENDER_SPRITES)
    }

    pub async fn run(&self) {
        // Not correctly updating v
        let mut chunk = [Rgb::new(); 8];
        let mut full_chunk = [Rgb::new(); 16];
        let mut frame = RgbFrame::new();
        let mut odd_frame = true;
        loop {
            // Render lines (-1, 0)
            for line in -1..240i32 {
                if line == 0 {
                    println!("!!!! Start of frame !!!!");
                }
                // Render line (line, 0)
                // Check if skip dot (0, 0)
                if odd_frame || line != 0 {
                    cycles!(self, 1);
                }

                // (-1, 1)
                if line == -1 {
                    println!("Pre-render line");
                    self.ppu_status.set(self.ppu_status.get() & !PpuStatus::VBLANK);
                } else {
                    println!("Rendered line");
                }

                // (line, 1)
                for tile in 0..32 {
                    if self.is_rendering() {
                        // Render chunk unless it's the pre-render line
                        if line != -1 && tile != 31 {
                            let drawn_chunk = self.apply_fine_x_scroll(&full_chunk);
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

            println!("!!!! Start of v-blank !!!!");
            // Post-render line (240, 0)
            cycles!(self, 340 + 1);
            // Post-render line (241, 1)
            self.ppu_status.set(self.ppu_status.get() | PpuStatus::VBLANK);
            cycles!(self, 340 + 341 * 19);

            self.frame_sender.send(frame.to_rgb_frame()).unwrap();

            odd_frame = !odd_frame;
        }
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
            Memory::PaletteRam => self.palette_ram.borrow()[addr]
        }
    }

    fn mmu_store(&self, addr: u16, val: u8) {
        let (mem, addr) = self.mmu_resolve(addr);
        match mem {
            Memory::ChrRom => (), // Do nothing if writing into ROM
            Memory::Ram => self.ram.borrow_mut()[addr] = val,
            Memory::PaletteRam => self.palette_ram.borrow_mut()[addr] =val
        }
    }

    fn render_line_chunk(&self) -> [Rgb; 8] {
        let v = self.v_reg.get().0;
        let mut rgb_chunk = [Rgb::new(); 8];
        let course_x = (v) & 0b11111;
        let course_y = (v >> 5) & 0b11111;
        let nt_sel = (v >> 10) & 0b11;
        let fine_y = (v >> 12) & 0b111;
        // Fetch nametable byte
        let nt = self.mmu_load((v & 0x0fff) | 0x2000) as u16;
        // Fetch attribute byte
        let at = self.mmu_load((course_x >> 2) | ((course_y >> 2) << 3) | (nt_sel << 6) | 0x23c0);
        // Fetch pattern bytes
        let half = self.ppu_ctrl.get().contains(PpuCtrl::BACKGROUND_SEL) as u16;
        let pattern_0 = self.mmu_load((half << 12) | (nt << 4) | 0x0 | (fine_y));
        let pattern_1 = self.mmu_load((half << 12) | (nt << 4) | 0x8 | (fine_y));
        // Fetch pixel colour

        for bit in 0..8 {
            let col_sel_0 = pattern_0 & (1 << bit) != 0;
            let col_sel_1 = pattern_1 & (1 << bit) != 0;
            rgb_chunk[bit] = match (col_sel_0, col_sel_1) {
                (false, false) => Rgb {red: 0, blue: 0, green: 0},
                (false, true)  => Rgb {red: 255, blue: 0, green: 0},
                (true, false)  => Rgb {red: 0, blue: 255, green: 0},
                (true, true)   => Rgb {red: 0, blue: 0, green: 255},
            }
        }

        rgb_chunk
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
                    println!("Writing {:02x} to PPU_SCROLL X", val);
                    t.set_x_course(val as u16);
                    let x_fine = 0b0111 & val;
                    self.x_fine_reg.set(x_fine);
                    t
                } else {
                    println!("Writing {:02x} to PPU_SCROLL Y", val);
                    t.set_y(val as u16)
                });
                self.write_toggle.set(!write_toggle);
            },
            6 => {
                let mut t = self.t_reg.get().0;
                if !write_toggle {
                    println!("Writing {:02x} to PPU_ADDR MSB", val);
                    t &= !0xff00; // Clear MSB byte
                    t |= ((val & 0b00111111) as u16) << 8; // Set MSB bytes but clear bits above bit 13
                } else {
                    println!("Writing {:02x} to PPU_ADDR LSB", val);
                    t &= !0x00ff;
                    t |= val as u16;
                };
                self.write_toggle.set(!write_toggle);
                let t = VramAddr(t);
                self.t_reg.set(t);
                self.v_reg.set(t);
            },
            7 => {
                println!("Writing to address: {:04x}", self.v_reg.get().0);
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
        self.oam_data.borrow_mut()[self.oam_addr.get() as usize]
    }

    pub fn write_oam(&self, val: u8) {
        let oam_addr = self.oam_addr.get();
        self.oam_data.borrow_mut()[oam_addr as usize] = val;
        self.oam_addr.set(oam_addr.wrapping_add(1));
    }
}