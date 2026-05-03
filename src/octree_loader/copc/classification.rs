use las::point::Classification;

pub fn classification_to_color(classification: &Classification) -> (u8, u8, u8) {
    match *classification {
        Classification::CreatedNeverClassified => (255, 255, 255), // white
        Classification::Unclassified => (200, 200, 200),           // light grey
        Classification::Ground => (139, 69, 19),                   // brown (ground)
        Classification::LowVegetation => (144, 238, 144),          // light green
        Classification::MediumVegetation => (34, 139, 34),         // medium green
        Classification::HighVegetation => (0, 100, 0),             // dark green
        Classification::Building => (255, 0, 0),                   // red
        Classification::LowPoint => (255, 0, 255),                 // magenta
        Classification::ModelKeyPoint => (255, 165, 0),            // orange
        Classification::Water => (0, 0, 255),                      // blue
        Classification::Rail => (128, 128, 128),                   // grey
        Classification::RoadSurface => (50, 50, 50),               // dark grey
        Classification::WireGuard => (255, 255, 0),                // yellow
        Classification::WireConductor => (255, 215, 0),            // gold
        Classification::TransmissionTower => (128, 0, 128),        // violet
        Classification::WireStructureConnector => (75, 0, 130),    // indigo
        Classification::BridgeDeck => (210, 180, 140),             // beige
        Classification::HighNoise => (0, 255, 255),                // cyan

        // Custom value
        Classification::Reserved(code) => {
            // pseudo-random palette based on code
            let r = (code.wrapping_mul(53)) % 255;
            let g = (code.wrapping_mul(97)) % 255;
            let b = (code.wrapping_mul(193)) % 255;
            (r, g, b)
        }
        Classification::UserDefinable(code) => {
            // another variation to distinguish
            let r = (code.wrapping_mul(29)) % 255;
            let g = (code.wrapping_mul(71)) % 255;
            let b = (code.wrapping_mul(157)) % 255;
            (r, g, b)
        }
    }
}
