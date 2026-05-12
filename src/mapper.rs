use std::cell::RefCell;
use std::rc::Rc;
use crate::parse_ines::INes;

mod mapper0;
mod mapper1;

pub trait Mapper {
    fn get(&mut self, loc: usize) -> u8;
    fn set(&mut self, loc: usize, val: u8);
    fn ppu_get(&mut self, loc: usize) -> u8;
    fn ppu_set(&mut self, loc: usize, val: u8);
}

pub fn generate_mapper(ines: INes) -> Rc<RefCell<dyn Mapper>> {
    match ines.mapper {
        0 => Rc::new(RefCell::new(mapper0::Mapper0::new(ines))),
        1 => Rc::new(RefCell::new(mapper1::Mapper1::new(ines))),
        _ => unimplemented!()
    }
}