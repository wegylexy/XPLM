use xplm::dataref::ReadOnly;

fn main() {
    let dr: Option<ReadOnly<f32>> = ReadOnly::<f32>::find("sim/does/not/matter");
    if let Some(dr) = dr {
        dr.set(1.0);
    }
}
