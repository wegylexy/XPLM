use xplm::dataref::ReadOnlyArray;

fn main() {
    let dr: Option<ReadOnlyArray<f32>> = ReadOnlyArray::<f32>::find("sim/does/not/matter");
    if let Some(dr) = dr {
        dr.set(0, &[1.0]);
    }
}
