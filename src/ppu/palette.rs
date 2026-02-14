macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        Colour::Rgb(Rgb($r, $g, $b))
    }
}

// Digital Prime Pallete (credit: https://www.firebrandx.com/nespalettes.html)
pub const PALETTE: [Colour; 64] = [
	rgb!(0x69, 0x69, 0x69), rgb!(0x00, 0x14, 0x8f), rgb!(0x1e, 0x02, 0x9b), rgb!(0x3f, 0x00, 0x8a), rgb!(0x60, 0x00, 0x60), rgb!(0x66, 0x00, 0x17), rgb!(0x57, 0x0d, 0x00), rgb!(0x45, 0x1b, 0x00),
	rgb!(0x24, 0x34, 0x00), rgb!(0x00, 0x42, 0x00), rgb!(0x00, 0x45, 0x00), rgb!(0x00, 0x3c, 0x1f), rgb!(0x00, 0x31, 0x5c), rgb!(0x00, 0x00, 0x00), rgb!(0x00, 0x00, 0x00), rgb!(0x00, 0x00, 0x00),
	rgb!(0xaf, 0xaf, 0xaf), rgb!(0x0f, 0x51, 0xdd), rgb!(0x44, 0x2f, 0xf3), rgb!(0x72, 0x20, 0xe2), rgb!(0xa3, 0x19, 0xb3), rgb!(0xae, 0x1c, 0x51), rgb!(0xa4, 0x34, 0x00), rgb!(0x88, 0x4d, 0x00),
	rgb!(0x67, 0x6d, 0x00), rgb!(0x20, 0x80, 0x00), rgb!(0x00, 0x8b, 0x00), rgb!(0x00, 0x7f, 0x42), rgb!(0x00, 0x6c, 0x97), rgb!(0x01, 0x01, 0x01), rgb!(0x00, 0x00, 0x00), rgb!(0x00, 0x00, 0x00),
	rgb!(0xff, 0xff, 0xff), rgb!(0x65, 0xaa, 0xff), rgb!(0x8c, 0x96, 0xff), rgb!(0xb9, 0x83, 0xff), rgb!(0xdd, 0x6f, 0xff), rgb!(0xea, 0x6f, 0xbd), rgb!(0xeb, 0x84, 0x66), rgb!(0xdc, 0xa2, 0x1f),
	rgb!(0xba, 0xb4, 0x03), rgb!(0x7e, 0xcb, 0x07), rgb!(0x54, 0xd3, 0x3e), rgb!(0x3c, 0xd2, 0x84), rgb!(0x3e, 0xc7, 0xcc), rgb!(0x4b, 0x4b, 0x4b), rgb!(0x00, 0x00, 0x00), rgb!(0x00, 0x00, 0x00),
	rgb!(0xff, 0xff, 0xff), rgb!(0xbd, 0xe2, 0xff), rgb!(0xce, 0xcf, 0xff), rgb!(0xe6, 0xc2, 0xff), rgb!(0xf6, 0xbc, 0xff), rgb!(0xf9, 0xc2, 0xed), rgb!(0xfa, 0xcf, 0xc6), rgb!(0xf8, 0xde, 0xac),
	rgb!(0xee, 0xe9, 0xa1), rgb!(0xd0, 0xf5, 0x9f), rgb!(0xbb, 0xf5, 0xaf), rgb!(0xb3, 0xf5, 0xcd), rgb!(0xb9, 0xed, 0xf0), rgb!(0xb9, 0xb9, 0xb9), rgb!(0x00, 0x00, 0x00), rgb!(0x00, 0x00, 0x00),
];

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy)]
pub enum Colour {
    Transparent,
    Rgb(Rgb),
}

#[derive(Clone, Copy)]
enum PaletteEntry {
    Transparent(u8),
    Colour(u8, u8)
}

struct Palette([PaletteEntry; 4]);

#[derive(Default)]
pub struct PaletteRam {
    palettes: [Palette; 4]
}

impl Rgb {
    pub fn new() -> Self {
        Rgb(0, 0, 0)
    }
}

impl Colour {
    fn unwrap_rgb(self) -> Rgb {
        if let Colour::Rgb(rgb) = self {
            rgb
        } else {
            panic!("Not an Rgb value")
        }
    }
}

impl PaletteEntry {
    fn unwrap_transparent(self) -> u8 {
        if let PaletteEntry::Transparent(entry) = self {
            entry
        } else {
            panic!("Not a Transparent value")
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Palette([
            PaletteEntry::Transparent(0),
            PaletteEntry::Colour(0, 0),
            PaletteEntry::Colour(0, 0),
            PaletteEntry::Colour(0, 0)
        ])
    }
}

impl PaletteRam {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn set(&mut self, loc: usize, val: u8) {
        let colour_id = loc & 0b00011;
        let palette_id = (loc & 0b01100) >> 2;
        let is_sprite = loc & &0b10000 != 0;
        let palette = &mut self.palettes[palette_id];
        let entry = &mut palette.0[colour_id];
        match entry {
            PaletteEntry::Transparent(col) => *col = val,
            PaletteEntry::Colour(bg_col, sprite_col ) => {
                if is_sprite {
                    *sprite_col = val;
                } else {
                    *bg_col = val;
                }
            }
        }
    }

    pub fn get(&self, loc: usize) -> u8 {
        let colour_id = loc & 0b00011;
        let palette_id = (loc & 0b01100) >> 2;
        let is_sprite = loc & &0b10000 != 0;
        let palette = &self.palettes[palette_id];
        let entry = &palette.0[colour_id];
        match entry {
            PaletteEntry::Transparent(col) => *col,
            PaletteEntry::Colour(bg_col, sprite_col ) => {
                if is_sprite {
                    *sprite_col
                } else {
                    *bg_col
                }
            }
        }
    }

    pub fn rgb_lookup(&self, palette_id: usize, colour_id: usize, is_sprite: bool) -> Colour {
        let palette = &self.palettes[palette_id];
        let colour = &palette.0[colour_id];
        match (colour, is_sprite) {
            (PaletteEntry::Transparent(_), _) => Colour::Transparent,
            (PaletteEntry::Colour(val, _), false) => PALETTE[*val as usize],
            (PaletteEntry::Colour(_, val), true) => PALETTE[*val as usize]
        }
    }

    pub fn background_colour(&self) -> Rgb {
        let colour = &self.palettes[0].0[0];
        let entry = colour.unwrap_transparent();
        PALETTE[entry as usize].unwrap_rgb()
    }
}