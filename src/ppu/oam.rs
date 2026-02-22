use std::rc::Rc;
use std::cell::RefCell;
use arrayvec::ArrayVec;
use super::palette::{Rgb, Colour, PaletteRam};
use super::Ppu;

use bitflags::bitflags;

bitflags! {
    /// Represents a set of flags.
    #[derive(Clone, Copy, Default)]
    pub struct OamAttributes: u8 {
        const PALETTE_0 = (1 << 0);
        const PALETTE_1 = (1 << 1);
        const PRIORITY = (1 << 5);
        const FLIP_HORIZONTALLY = (1 << 6);
        const FLIP_VERTICALLY = (1 << 7);
    }
}

#[derive(Clone, Copy, Default)]
struct Sprite {
    y_pos: u8,
    tile_index: u8,
    attributes: OamAttributes,
    x_pos: u8
}

type SecondaryOam = ArrayVec<(Sprite, bool), 8>;
pub struct Oam {
    chr: Rc<RefCell<Vec<u8>>>,
    palette_ram: Rc<RefCell<PaletteRam>>,
    primary: [Sprite; 64],
    secondary: SecondaryOam,
}

impl Sprite {
    fn on_scanline(&self, y: u8) -> bool {
        if y >= self.y_pos && y < self.y_pos + 8 {
            true
        } else {
            false
        }
    }
}

impl Sprite {
    fn new() -> Self {
        Default::default()
    }
}

impl Oam {
    pub fn new(chr: Rc<RefCell<Vec<u8>>>, palette_ram: Rc<RefCell<PaletteRam>>) -> Self {
        Self {
            chr: chr,
            palette_ram: palette_ram,
            primary: [Sprite::new(); 64],
            secondary: SecondaryOam::from([(Sprite::new(), false); 8]),
        }
    }

    pub fn write(&mut self, addr: usize, val: u8) {
        let addr = 0xff & addr;
        let entry_index = addr / 4;
        let entry = &mut self.primary[entry_index];
        let part = addr % 4;
        match part {
            0 => entry.y_pos = val,
            1 => entry.tile_index = val,
            2 => entry.attributes = OamAttributes::from_bits_truncate(val),
            3 => entry.x_pos = val,
            _ => unreachable!()
        };
    }

    pub fn read(&self, addr: usize) -> u8 {
        let addr = 0xff & addr;
        let entry_index = addr / 4;
        let entry = &self.primary[entry_index];
        let part = addr % 4;
        match part {
            0 => entry.y_pos,
            1 => entry.tile_index,
            2 => entry.attributes.bits(),
            3 => entry.x_pos,
            _ => unreachable!()
        }
    }

    pub fn clear_secondary_oam(&mut self) {
        self.secondary.clear();
    }

    /// Adds sprites to secondary OAM and indicates overflow if detected
    /// TODO: Does not perform NES' original "incorrect" overflow check
    pub fn populate_secondary_oam(&mut self, y: usize) -> bool {
        for (i, sprite) in self.primary.iter().enumerate() {
            if sprite.on_scanline(y as u8) {
                let sprite_zero = i == 0;
                if self.secondary.try_push((*sprite, sprite_zero)).is_err() {
                    return true;
                }
            }
        }
        false
    }

    /// Draws a 8-pixel chunk of RGB data. This is intended to be aligned
    /// with the output from the background drawing
    pub fn draw_chunk(&self, bank_sel: bool, y: usize, x_course: usize) -> [Colour; 8] {
        // Constrain values to help with compiler optimisations
        let y = (y & 0xff) as i32;
        let x_course = (x_course & 0x1f) as i32;
        let mut pixels = [Colour::new(); 8];
        for (sprite, sprite_zero) in self.secondary.iter() {
            let chr = self.chr.borrow();
            let tile_index = sprite.tile_index as usize;
            let addr = (tile_index << 1) | ((bank_sel as usize) << 3);
            let pattern = &chr[addr..(addr + 16)];
            let sprite_y = y - sprite.y_pos as i32;
            for (x, pixel) in pixels.iter_mut().enumerate() {
                let palette_id = sprite.attributes & (OamAttributes::PALETTE_0 | OamAttributes::PALETTE_1);
                let palette_id = palette_id.bits() as usize;
                // Determine pixel's colour index from pattern
                let sprite_x = (x_course * 8) + (x as i32) - sprite.x_pos as i32;
                *pixel = if sprite_x > 0 && sprite_x < 8 {
                    let sprite_y = sprite_y as usize;
                    let bit0 = pattern[sprite_y] & (1 << sprite_x) != 0;
                    let bit1 = pattern[sprite_y + 8] & (1 << sprite_x) != 0;
                    let colour_id = (bit0 as usize) | ((bit1 as usize) << 1);
                    // Get colour from palette
                    let palette_ram: std::cell::Ref<'_, PaletteRam> = self.palette_ram.borrow();
                    let mut colour = palette_ram.rgb_lookup(palette_id, colour_id, true);
                    if *sprite_zero {
                        colour = colour.to_sprite0();
                    }
                    colour
                } else {
                    Colour::Transparent
                }
            }
        }
        pixels
    }
}
