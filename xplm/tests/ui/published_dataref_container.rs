#[derive(xplm::PublishedDataRefContainer)]
#[dataref_prefix = "MyAvionics/Nav1/"]
struct NavRadio {
    frequency_khz: i32, // -> "MyAvionics/Nav1/frequency_khz"
    #[writable]
    course_deg: f32, // -> "MyAvionics/Nav1/course_deg"
    #[dataref = "MyAvionics/Shared/active_nav_ident"] // explicit: skips the prefix
    #[writable]
    ident: Vec<u8>,
}

// The consumer side, reading back what `NavRadio` above publishes — plain
// #[derive(DataRefContainer)] fields, same as any other plugin reading any
// other dataref, built-in or not.
#[derive(xplm::DataRefContainer)]
struct NavRadioReader {
    #[dataref = "MyAvionics/Nav1/frequency_khz"]
    frequency_khz: i32,
    #[dataref = "MyAvionics/Shared/active_nav_ident"]
    #[writable]
    ident: Vec<u8>,
}

fn main() {
    // Not called (would need a hosted X-Plane process to succeed) — this
    // only proves both derives expand to the right shapes, and that the
    // accessor methods actually type-check.
    let _: fn(NavRadio) -> Option<NavRadioHandle> = NavRadio::publish;
    let _: fn() -> Option<NavRadioReaderHandle> = NavRadioReader::find;

    fn use_handle(handle: &NavRadioHandle) {
        let _frequency: i32 = handle.frequency_khz(); // getter, even though not #[writable]
        let _course: f32 = handle.course_deg();
        handle.set_course_deg(90.0); // setter, only exists because #[writable]
        let _ident: Vec<u8> = handle.ident();
        handle.set_ident(b"KABC".to_vec());

        let cloned = handle.clone(); // Handle is cheap to Clone (shares the same state)
        let _ = cloned.frequency_khz();
    }
    let _ = use_handle;

    fn use_reader(reader: &NavRadioReaderHandle) {
        let _frequency: i32 = reader.frequency_khz();
        reader.set_ident(b"KABC".to_vec()); // only compiles because ident was #[writable]
        let _bytes: Vec<u8> = reader.ident();
    }
    let _ = use_reader;
}
