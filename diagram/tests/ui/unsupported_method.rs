const DIAGRAM: &str = sparsley_diagram::diagram! {
    let mut map = SparseMap::from([(3, 'a'), (7, 'b')]);
    map.drain();
};

fn main() {}
