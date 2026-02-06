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

struct Pattern {
    data: [PalletId; 64]
}

impl Pattern {
    fn new() -> Self {
        Pattern {
            data: [PalletId::Transparent; 64]
        }
    }

    fn parse(vram: &[u8]) -> Self {
        let mut pattern = Pattern::new();
        for i in 0..8 {
            let plane_part1 = vram[i];
            let plane_part2 = vram[i+8];
            for bit_pos in 0..8 {
                let bit1 = plane_part1 & (1 << bit_pos) != 0;
                let bit2 = plane_part2 & (1 << bit_pos) != 0;
                pattern.data[i*8 + bit_pos] = match (bit1, bit2) {
                    (false, false) => PalletId::Transparent,
                    (false, true)  => PalletId::Pallet1,
                    (true, false)  => PalletId::Pallet2,
                    (true, true)   => PalletId::Pallet3
                };
            }
        }
        pattern
    }
}

pub struct Ppu {
    ppu_ctrl: Cell<PpuCtrl>,
    ppu_mask: Cell<PpuMask>,
    ppu_status: Cell<PpuStatus>,
    oam_addr: Cell<u8>,
    ppu_scroll_x: Cell<u8>,
    ppu_scroll_y: Cell<u8>,
    ppu_addr: Cell<u16>,
    write_toggle: Cell<bool>,
    clock: Rc<RefCell<Clock>>,
    chr_rom: RefCell<Vec<u8>>,
    ram: RefCell<Vec<u8>>,
    oam_data: RefCell<Vec<u8>>,
    frame_sender: mpsc::Sender<Vec<u8>>
}

enum Memory {
    ChrRom,
    Ram
}


fn compute_fine_x_scroll(full_chunk: &[Rgb; 16], fine_x: u8) -> [Rgb; 8] {
    let fine_x = fine_x as usize;
    let mut output_chunk = [Rgb::new(); 8];
    output_chunk.copy_from_slice(&full_chunk[fine_x..(fine_x + 8)]);
    output_chunk
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
            ppu_scroll_x: Cell::new(0),
            ppu_scroll_y: Cell::new(0),
            ppu_addr: Cell::new(0),
            write_toggle: Cell::new(false),
            oam_data: RefCell::new(Vec::new()), // TODO change this, just needed to compile
            chr_rom: RefCell::new(chr_rom),
            ram: RefCell::new(vec![0; 4096]),
            frame_sender: frame_sender
        }
    }

    pub async fn run(&self) {
        let mut full_chunk = [Rgb::new(); 16];
        let mut frame = RgbFrame::new();
        let mut odd_frame = true;
        loop {
            println!("!!!! Start of frame !!!!");
            let (mut v, fine_x) = self.get_t_x();
            // Pre-render (-1, 0)
            cycles!(self, 1);
            // Pre-render (-1, 1)
            self.ppu_status.set(self.ppu_status.get() & !PpuStatus::VBLANK);
            cycles!(self, 320);
            // Pre-render (-1, 321) - prefetch next two tiles
            let mut chunk0 = self.render_line_chunk(v);
            v += 1;
            let mut chunk1 = self.render_line_chunk(v);
            v += 1;
            full_chunk[0..8].copy_from_slice(&chunk0[0..8]);
            full_chunk[8..16].copy_from_slice(&chunk1[0..8]);
            cycles!(self, 16 + 4);

            // Visible lines (0, 0)
            for l in 0..240 {
                // Render line (n, 0)
                // Check if skip dot (0, 0)
                if odd_frame || l > 0 {
                    cycles!(self, 1);
                }

                for i in 0..32 {
                    // Render chunk
                    let drawn_chunk = compute_fine_x_scroll(&full_chunk, fine_x);
                    let start = i*8;
                    let end = start + 8;
                    frame.write_chunk(l, i, drawn_chunk);

                    // Get next chunk
                    chunk0 = chunk1;
                    chunk1 = self.render_line_chunk(v);
                    v += 1;
                    full_chunk[0..8].copy_from_slice(&chunk0[0..8]);
                    full_chunk[8..16].copy_from_slice(&chunk1[0..8]);
                    cycles!(self, 8);
                }
                // (, 255) Nothing really happens for the rest of the line
                cycles!(self, 84);
            }

            // Post-render line (240, 0)
            cycles!(self, 340 + 1);
            // Post-render line (241, 1)
            self.ppu_status.set(self.ppu_status.get() | PpuStatus::VBLANK);
            cycles!(self, 340 + 341 * 19);

            self.frame_sender.send(frame.to_rgb_frame());

            odd_frame = !odd_frame;
        }
    }

    fn mmu_resolve(&self, addr: u16) -> (Memory, usize) {
        let (mem, loc) = match addr {
            0x0000..0x2000 => (Memory::ChrRom, addr),
            0x2000..0x4000 => (Memory::Ram, addr - 0x2000),
                         _ => panic!("Unexpected vram access")
        };
        (mem, loc as usize)
    }

    fn mmu_load(&self, addr: u16) -> u8 {
        // println!("Loading from 0x{:04x}", addr);
        let (mem, addr) = self.mmu_resolve(addr);
        match mem {
            Memory::ChrRom => self.chr_rom.borrow()[addr],
            Memory::Ram => self.ram.borrow()[addr]
        }
    }

    fn mmu_store(&self, addr: u16, val: u8) {
        let (mem, addr) = self.mmu_resolve(addr);
        match mem {
            Memory::ChrRom => (), // Do nothing if writing into ROM
            Memory::Ram => self.ram.borrow_mut()[addr] = val
        }
    }

    fn render_line_chunk(&self, v: u16) -> [Rgb; 8] {
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

    fn vram_increment(&self) {
        let vram_incr_flag = self.ppu_ctrl.get().contains(PpuCtrl::VRAM_INCR);
        self.ppu_addr.set(self.ppu_addr.get().wrapping_add(
            if vram_incr_flag {
                32
            } else {
                1
            })
        );
    }

    fn write_vram(&self, val: u8) {
        let addr = self.ppu_addr.get();
        self.chr_rom.borrow_mut()[addr as usize] = val;
        self.vram_increment();
    }

    fn read_vram(&self) -> u8 {
        let addr = self.ppu_addr.get();
        let val = self.chr_rom.borrow_mut()[addr as usize];
        self.vram_increment();
        val
    }

    fn read_status(&self) -> u8 {
        self.write_toggle.set(false);
        self.ppu_status.get().bits()
    }

    fn get_t_x(&self) -> (u16, u8) {
        let scroll_x = self.ppu_scroll_x.get() as u16;
        let scroll_y = self.ppu_scroll_y.get() as u16;
        let t =
            ((scroll_x & 0b11111000) >> 3) |
            ((scroll_y & 0b11111000) << 2) |
            ((scroll_y & 0b00000111) << 13);
        let x = scroll_x as u8 & 0b111;
        (t, x)
    }

    pub fn set_reg(&self, addr: usize, val: u8) {
        let write_toggle = self.write_toggle.get();
        match addr {
            0 => self.ppu_ctrl.set(PpuCtrl::from_bits_retain(val)),
            1 => self.ppu_mask.set(PpuMask::from_bits_retain(val)),
            2 => (),
            3 => self.oam_addr.set(val),
            4 => self.write_oam(val),
            5 => {
                if !write_toggle {
                    self.ppu_scroll_x.set(val)
                } else {
                    self.ppu_scroll_y.set(val)
                }
                self.write_toggle.set(!self.write_toggle.get());
            },
            6 => {
                self.ppu_addr.set(if !write_toggle {
                    (val as u16) << 8
                } else {
                    val as u16
                });
                self.write_toggle.set(!self.write_toggle.get());
            },
            7 => self.write_vram(val),
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
            7 => self.read_vram(),
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