//! The height of a waypoint, whatever the frame it is written in.
//!
//! A waypoint carries its height in one of three frames, and the three mean different
//! things:
//!
//! * **above the sea** — the height the vehicle flies at, told against the sea;
//! * **above the home** — the height the vehicle climbs to after it leaves the home;
//! * **above the ground** — the height the vehicle flies at over the ground it is over.
//!
//! The frame travels with the item, and the vehicle is the one that decides what to do
//! with it. The profile, on the other hand, has to draw the plan and the ground on the
//! same scale, and for that the height of every waypoint has to be told against the sea.

/// `MAV_FRAME_GLOBAL`: the height is above the sea.
pub const FRAME_ABOVE_SEA: u32 = 0;

/// `MAV_FRAME_GLOBAL_RELATIVE_ALT`: the height is above the home.
pub const FRAME_ABOVE_HOME: u32 = 3;

/// `MAV_FRAME_GLOBAL_TERRAIN_ALT`: the height is above the ground.
pub const FRAME_ABOVE_GROUND: u32 = 10;

/// The name a frame is known by.
#[must_use]
pub fn name_of_frame(frame: u32) -> &'static str {
    match frame {
        FRAME_ABOVE_SEA => "above the sea",
        FRAME_ABOVE_HOME => "above the home",
        FRAME_ABOVE_GROUND => "above the ground",
        _ => "in an unknown frame",
    }
}

/// The height of a waypoint above the sea, whatever the frame it is written in.
///
/// The height of the home and the height of the ground are what the two relative frames
/// are measured from. A frame that needs one of them and does not have it cannot be
/// converted, and the answer is that it cannot: a ground station that guesses the
/// height of the home flies the aircraft into the ground.
///
/// # Examples
///
/// ```
/// use mpcraft_mission::altitude::{above_sea, FRAME_ABOVE_GROUND, FRAME_ABOVE_SEA};
///
/// // A height above the sea is the height itself.
/// assert_eq!(above_sea(FRAME_ABOVE_SEA, 120.0, None, None), Some(120.0));
///
/// // A height above the ground needs the ground.
/// assert_eq!(above_sea(FRAME_ABOVE_GROUND, 30.0, None, Some(570.0)), Some(600.0));
/// assert_eq!(above_sea(FRAME_ABOVE_GROUND, 30.0, None, None), None);
/// ```
#[must_use]
pub fn above_sea(frame: u32, height: f64, home: Option<f64>, ground: Option<f64>) -> Option<f64> {
    match frame {
        FRAME_ABOVE_SEA => Some(height),
        FRAME_ABOVE_HOME => home.map(|home| home + height),
        FRAME_ABOVE_GROUND => ground.map(|ground| ground + height),
        // A frame nobody knows is the height as it is: it is the least that can be said
        // about it, and the vehicle will refuse it if it does not like it.
        _ => Some(height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_height_above_the_sea_is_the_height_itself() {
        assert_eq!(above_sea(FRAME_ABOVE_SEA, 120.0, None, None), Some(120.0));
    }

    #[test]
    fn a_height_above_the_home_needs_the_home() {
        assert_eq!(
            above_sea(FRAME_ABOVE_HOME, 30.0, Some(584.0), None),
            Some(614.0)
        );
        assert_eq!(above_sea(FRAME_ABOVE_HOME, 30.0, None, None), None);
    }

    #[test]
    fn a_height_above_the_ground_needs_the_ground() {
        assert_eq!(
            above_sea(FRAME_ABOVE_GROUND, 30.0, None, Some(570.0)),
            Some(600.0)
        );
        assert_eq!(above_sea(FRAME_ABOVE_GROUND, 30.0, None, None), None);
    }

    #[test]
    fn the_frames_are_named_as_the_specification_names_them() {
        assert_eq!(name_of_frame(FRAME_ABOVE_SEA), "above the sea");
        assert_eq!(name_of_frame(FRAME_ABOVE_HOME), "above the home");
        assert_eq!(name_of_frame(FRAME_ABOVE_GROUND), "above the ground");
    }
}
