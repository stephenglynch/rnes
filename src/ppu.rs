use bitflags::bitflags;

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
    ppu_ctrl: PpuCtrl,
    ppu_mask: PpuMask,
    ppu_status: PpuStatus,
    oam_addr: u8,
    ppu_scroll_x: u8,
    ppu_scroll_y: u8,
    ppu_addr: u16,
    write_toggle: bool,
    chr_rom: Vec<u8>,
    oam_data: Vec<u8>
}

impl Ppu {
    pub fn new(chr_rom: Vec<u8>) -> Self {
        Self {
            //TODO: To be more accurate these should initialise to random values
            ppu_ctrl: PpuCtrl::from_bits_retain(0),
            ppu_mask: PpuMask::from_bits_retain(0),
            ppu_status: PpuStatus::from_bits_retain(0),
            oam_addr: 0,
            oam_data: 0,
            ppu_scroll_x: 0,
            ppu_scroll_y: 0,
            ppu_addr: 0,
            write_toggle: false,
            chr_rom: chr_rom
        }
    }

    fn vram_increment(&mut self) {
        self.ppu_addr = self.ppu_addr.wrapping_add(
            if self.ppu_ctrl.contains(PpuCtrl::VRAM_INCR) {
                32
            } else {
                1
            }
        );
    }

    fn write_vram(&mut self, val: u8) {
        self.chr_rom[self.ppu_addr as usize] = val;
        self.vram_increment();
    }

    fn read_vram(&mut self) -> u8 {
        let val = self.chr_rom[self.ppu_addr as usize];
        self.vram_increment();
        val
    }

    fn read_ppu_status(&mut self) -> u8 {
        self.write_toggle = false;
        self.ppu_status.bits()
    }

    pub fn set_reg(&mut self, addr: usize, val: u8) {
        match addr {
            0 => self.ppu_ctrl = PpuCtrl::from_bits_retain(val),
            1 => self.ppu_mask = PpuMask::from_bits_retain(val),
            2 => (),
            3 => self.oam_addr = val,
            4 => self.write_oam(val),
            5 => if !self.write_toggle { self.ppu_scroll_x = val } else { self.ppu_scroll_y = val },
            6 => self.ppu_addr = if !self.write_toggle { (val as u16) << 8 } else { val as u16 },
            7 => self.write_vram(val),
            _ => unreachable!()
        }
    }

    pub fn get_reg(&mut self, addr: usize) -> u8 {
        match addr {
            0 => 0,
            1 => 0,
            2 => self.read_ppu_status(),
            3 => 0,
            4 => self.read_oam(),
            5 => 0,
            6 => 0,
            7 => self.read_vram(),
            _ => unreachable!()
        }
    }

    fn read_oam(&self) -> u8 {
        self.oam_data[self.oam_addr as usize]
    }

    pub fn write_oam(&mut self, val: u8) {
        self.oam_data[self.oam_addr as usize] = val;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }
}