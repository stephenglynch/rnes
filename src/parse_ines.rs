use std::fs::read;
use bitflags::bitflags;

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Flags6: u8 {
        const NAME_TABLE_ARRANGEMENT = (1 << 0);
        const HAS_MEMORY = (1 << 1);
        const TRAINER_PRESENT = (1 << 2);
        const ALTERNATIVE_NAME_TABLE = (1 << 3);
    }

    struct Flags7: u8 {
        const VS_UNISYSTEM = (1 << 0);
        const PLAY_CHOICE = (1 << 1);
        const NES_VERSION_BIT0 = (1 << 2);
        const NES_VERSION_BIT1 = (1 << 3);
        const NES_VERSION = Self::NES_VERSION_BIT1.bits() | Self::NES_VERSION_BIT1.bits();
    }
}

pub struct INes {
    prg_rom: Vec<u8>,
    mapper: u16,
    submapper: u8,
    trainer: Option<Vec<u8>>,
    chr_rom: Option<Vec<u8>>,
}

pub fn read_ines(filename: &str) -> Option<INes> {
    let raw = read(filename).unwrap();

    // Check if file contains magic sequence
    if &raw[0..4] != b"NES\x1a" {
        return Option::None;
    }

    let flags6 = Flags6::from_bits(raw[6] & 0x0f)?;
    let flags7 = Flags7::from_bits(raw[7] & 0x0f)?;

    let nes2 = (flags7 & Flags7::NES_VERSION).contains(Flags7::NES_VERSION_BIT1);

    let trainer_used = flags6.contains(Flags6::TRAINER_PRESENT);
    let trainer_size = if trainer_used {512} else {0};
    let trainer_start = 16;
    let trainer_end = trainer_start + trainer_size;
    let trainer = if trainer_used {
        Some(raw[trainer_start..trainer_end].to_vec())
    } else {
        Option::None
    };

    let prg_rom_size_lsb = (raw[4] as usize);
    let prg_rom_size_msb = if nes2 {
        ((raw[9] & 0x0f) as usize) << 8
    } else {
        0
    };
    let prg_rom_size = (prg_rom_size_msb | prg_rom_size_lsb) * 16384;
    let prg_rom_start = trainer_end;
    let prg_rom_end = prg_rom_start + prg_rom_size;
    let prg_rom = raw[prg_rom_start..prg_rom_end].to_vec();

    let chr_rom_size_lsb = (raw[5] as usize);
    let chr_rom_size_msb = if nes2 {
        (raw[9] & 0xf0) as usize
    } else {
        0
    };
    let chr_rom_size = (chr_rom_size_lsb | chr_rom_size_msb) * 8192;
    let chr_rom_start = prg_rom_end;
    let chr_rom_end = chr_rom_start + chr_rom_size;
    let chr_rom = if chr_rom_size != 0 {
        Some(raw[chr_rom_start..chr_rom_end].to_vec())
    } else {
        Option::None
    };

    let mapper_lo = (raw[6] & 0xf0) >> 4 & (raw[7] & 0xf0);
    let mapper_hi = raw[8] & 0x0f;
    let mapper = (mapper_hi as u16) << 8 | (mapper_lo as u16);
    let submapper = (raw[8] & 0xf0) >> 4;

    Some(INes {
        prg_rom: prg_rom,
        mapper: mapper,
        submapper: submapper,
        trainer: trainer,
        chr_rom: chr_rom,
    })
}