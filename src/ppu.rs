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
    fn set_x_scroll_course(self, val: u8) -> Self {
        let mut v = self.0;
        v &= !0b00000000_00011111; // Clear x-scroll bits
        v |= (val as u16 & 0b11111000) >> 3;
        VramAddr(v)
    }

    fn set_x_scroll(self, val: u8) -> (Self, u8) {
        (self.set_x_scroll_course(val), val as u8 & 0b111)
    }

    fn set_y_scroll(self, val: u8) -> Self {
        let val = val as u16;
        let mut v = self.0;
        v &= !0b01110011_11100000; // Clear y-scroll bits
        v |= (val & 0b11111000) << 2;
        v |= (val & 0b00000111) << 12;
        VramAddr(v)
    }

    fn to_x_scroll_course(&self) -> u8 {
        (self.0 << 3) as u8
    }

    fn to_y_scroll(&self) -> u8 {
        let mut y = 0;
        y |= (self.0 & 0b01110000_00000000) >> 12;
        y |= (self.0 & 0b00000011_11100000) >> 5;
        y as u8
    }

    fn add_x_scroll_course(self, val: u8) -> Self {
        let val = self.to_x_scroll_course() + val;
        self.set_x_scroll_course(val)
    }

    fn add_y_scroll(self, val: u8) -> Self {
        let val = self.to_y_scroll() + val;
        self.set_y_scroll(val)
    }

    fn set_x_scroll_with_addr(&self, other: Self) -> Self {
        let mut new_v = self.0 & !0b00000000_00011111;
        let other = other.0;
        new_v |= other & 0b00000000_00011111;
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
        output_chunk
    }

    fn v_hor_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_x_scroll_course(1));
    }

    fn v_ver_inc(&self) {
        self.v_reg.set(self.v_reg.get().add_y_scroll(1));
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

    pub async fn run(&self) {
        // Not correctly updating v
        let mut full_chunk = [Rgb::new(); 16];
        let mut frame = RgbFrame::new();
        let mut odd_frame = true;
        loop {
            println!("!!!! Start of frame !!!!");
            // Pre-render (-1, 0)
            cycles!(self, 1);
            // Pre-render (-1, 1)
            self.ppu_status.set(self.ppu_status.get() & !PpuStatus::VBLANK);
            cycles!(self, 320);
            // Pre-render (-1, 321) - prefetch next two tiles
            let mut chunk0 = self.render_line_chunk();
            self.v_hor_inc();
            let mut chunk1 = self.render_line_chunk();
            self.v_hor_inc();
            full_chunk[0..8].copy_from_slice(&chunk0[0..8]);
            full_chunk[8..16].copy_from_slice(&chunk1[0..8]);
            cycles!(self, 16 + 4);

            // Visible lines (0, 0)
            for l in 0..240 {
                // Render line (l, 0)
                // Check if skip dot (0, 0)
                if odd_frame || l > 0 {
                    cycles!(self, 1);
                }

                // (l, 1)
                for i in 0..32 {
                    // Render chunk
                    let drawn_chunk = self.apply_fine_x_scroll(&full_chunk);
                    frame.write_chunk(l, i, drawn_chunk);

                    // Get next chunk
                    chunk0 = chunk1;
                    chunk1 = self.render_line_chunk();
                    full_chunk[0..8].copy_from_slice(&chunk0[0..8]);
                    full_chunk[8..16].copy_from_slice(&chunk1[0..8]);
                    cycles!(self, 8);
                    self.v_hor_inc();
                }
                self.v_ver_inc();
                cycles!(self, 1);

                // (l, 257) hori(v) = hori(t)
                self.reset_v_hor();

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
                let mut t = self.t_reg.get();
                self.t_reg.set(if !write_toggle {
                    println!("Writing {:02x} to PPU_SCROLL X", val);
                    let (t, x_fine) = t.set_x_scroll(val);
                    self.x_fine_reg.set(x_fine);
                    t
                } else {
                    println!("Writing {:02x} to PPU_SCROLL Y", val);
                    t.set_y_scroll(val)
                });
                self.write_toggle.set(!write_toggle);
            },
            6 => {
                let mut t = self.t_reg.get().0;
                if !write_toggle {
                    println!("Writing {:02x} to PPU_ADDR MSB", val);
                    t &= !0xff00;
                    t |= (val as u16) << 8;
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