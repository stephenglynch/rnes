use std::cell::{Cell, RefCell, Ref, RefMut};
use std::rc::Rc;
use bitflags::bitflags;
use crate::clock::{Clock, CycleDelay};

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
        const BACKGROUND_EN = (1 << 4);
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
    oam_data: RefCell<Vec<u8>>
}

impl Ppu {
    pub fn new(clock: Rc<RefCell<Clock>>, chr_rom: Vec<u8>) -> Self {
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
            chr_rom: RefCell::new(chr_rom)
        }
    }

    pub async fn run(&self) {
        let scan_line = 341;
        loop {
            // Render (0, 0)
            cycles!(self, 241 * scan_line + 1);
            // TODO

            // V-blank (241, 1)
            self.ppu_status.set(self.ppu_status.get() | PpuStatus::VBLANK);
            cycles!(self, 20 * scan_line);

            // Pre-render (261, 1)
            self.ppu_status.set(self.ppu_status.get() & !PpuStatus::VBLANK);
            cycles!(self, scan_line - 1);
        }
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