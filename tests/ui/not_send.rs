use std::rc::Rc;

use sparsley::{SparseMap, map};

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<SparseMap<u32, Rc<u8>>>();
    assert_send::<map::IntoIter<u32, Rc<u8>>>();
}
