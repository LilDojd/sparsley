const DIAGRAM: &str = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
    map.swap_indices(0, 2);
};

fn main() {}
