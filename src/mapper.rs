use std::cell::RefCell;
use std::rc::Rc;
use crate::parse_ines::INes;

mod mapper0;

pub trait Mapper {
    fn get(&mut self, loc: usize) -> u8;
    fn set(&mut self, loc: usize, val: u8);
    fn ppu_get(&mut self, loc: usize) -> u8;
    fn ppu_set(&mut self, loc: usize, val: u8);
}

pub fn generate_mapper(ines: INes) -> Rc<RefCell<dyn Mapper>> {
    Rc::new(RefCell::new(mapper0::Mapper0::new(ines)))
}