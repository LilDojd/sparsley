const DIAGRAM: &str = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
    map.insert(1, 2);
};

fn main() {}
