use std::rc::Rc;
use std::cell::RefCell;
use arrayvec::ArrayVec;
use crate::mapper::Mapper;
use super::palette::{Colour, PaletteRam};

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

type SecondaryOam = ArrayVec<(Sprite, usize), 8>;
pub struct Oam {
    mapper: Rc<RefCell<dyn Mapper>>,
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
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>, palette_ram: Rc<RefCell<PaletteRam>>) -> Self {
        Self {
            mapper: mapper,
            palette_ram: palette_ram,
            primary: [Sprite::new(); 64],
            secondary: ArrayVec::new(),
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
                if self.secondary.try_push((*sprite, i)).is_err() {
                    return true;
                }
            }
        }
        false
    }

    fn get_pattern(&self, start_addr: usize) -> [u8; 16] {
        let mut mapper = self.mapper.borrow_mut();
        let mut pattern = [0; 16];
        for (i, addr) in  (start_addr..(start_addr + 16)).enumerate() {
            pattern[i] = mapper.ppu_get(addr);
        }
        pattern
    }

    /// Draws a 8-pixel chunk of RGB data. This is intended to be aligned
    /// with the output from the background drawing
    pub fn draw_chunk(&self, bank_sel: bool, y: usize, x_course: usize) -> [Colour; 8] {
        // Constrain values to help with compiler optimisations
        let y = (y & 0xff) as i32;
        let x_course = (x_course & 0x1f) as i32;
        let mut pixels = [Colour::new(); 8];
        for (sprite, sprite_num) in self.secondary.iter() {
            let flip_x = sprite.attributes.contains(OamAttributes::FLIP_HORIZONTALLY);
            let flip_y = sprite.attributes.contains(OamAttributes::FLIP_VERTICALLY);
            let mut sprite_y = y - sprite.y_pos as i32;
            if flip_y {
                sprite_y = 7 - sprite_y;
            }
            let tile_index = sprite.tile_index as usize;
            let addr = (tile_index << 4) | ((bank_sel as usize) << 12);
            let pattern = self.get_pattern(addr);
            assert!(sprite_y >= 0 && sprite_y < 8);
            for (x, pixel) in pixels.iter_mut().enumerate() {
                let palette_id = sprite.attributes & (OamAttributes::PALETTE_0 | OamAttributes::PALETTE_1);
                let palette_id = palette_id.bits() as usize;
                // Determine pixel's colour index from pattern
                let mut sprite_x = (x_course * 8) + (x as i32) - sprite.x_pos as i32;
                let new_pixel = if sprite_x >= 0 && sprite_x < 8 {
                    if !flip_x {
                        sprite_x = 7 - sprite_x;
                    }
                    let sprite_y = sprite_y as usize;
                    let bit0 = pattern[sprite_y] & (1 << sprite_x) != 0;
                    let bit1 = pattern[sprite_y + 8] & (1 << sprite_x) != 0;
                    let colour_id = (bit0 as usize) | ((bit1 as usize) << 1);
                    // Get colour from palette
                    let palette_ram: std::cell::Ref<'_, PaletteRam> = self.palette_ram.borrow();
                    let mut colour = palette_ram.rgb_lookup(palette_id, colour_id, true);
                    if *sprite_num == 0 {
                        colour = colour.to_sprite0();
                    }
                    colour
                } else {
                    Colour::Transparent
                };
                *pixel = pixel.combine(new_pixel);
            }
        }
        pixels
    }
}
