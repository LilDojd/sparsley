use sparsley::SparseMap;

fn main() {
    let mut map = SparseMap::from([(1u32, 'a')]);
    let (keys, values) = map.as_mut_slices();
    values[0] = 'b';
    keys[0] = 2;
}
