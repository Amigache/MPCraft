//! What the outputs of a vehicle are for.
//!
//! An output of an autopilot is told what to do by a parameter: `SERVO1_FUNCTION` says
//! that the first output drives a motor, and `SERVO5_FUNCTION` says that the fifth one
//! is a light. The numbers are the ones the specification gives them, and the names are
//! the ones the operator knows them by.

/// The name of a function, when the specification gives it one.
///
/// The numbers that the specification does not name are `None`: a function that is
/// named wrongly is worse than a function that is not named at all.
#[must_use]
pub fn servo_function_name(value: f64) -> Option<&'static str> {
    let number = value.round();

    if number < 0.0 {
        return None;
    }

    let number = number as u32;

    match number {
        0 => Some("Disabled"),
        1 => Some("Motor"),
        2..=39 => Some("Servo"),
        40 => Some("Light"),
        41 => Some("Beacon"),
        42 => Some("Camera control"),
        43 => Some("Camera pitch"),
        44 => Some("Camera yaw"),
        45 => Some("Telemetry transmit"),
        46 => Some("Telemetry receive"),
        69..=128 => Some("Motor"),
        _ => None,
    }
}

/// The name of the motor a function drives, when it drives one.
///
/// The motors are numbered from one, the way the parameters that name them are.
#[must_use]
pub fn motor_number(value: f64) -> Option<u32> {
    let number = value.round();

    if !(69.0..=128.0).contains(&number) {
        return None;
    }

    Some(number as u32 - 68)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_functions_the_specification_names_are_named() {
        assert_eq!(servo_function_name(0.0), Some("Disabled"));
        assert_eq!(servo_function_name(1.0), Some("Motor"));
        assert_eq!(servo_function_name(2.0), Some("Servo"));
        assert_eq!(servo_function_name(40.0), Some("Light"));
        assert_eq!(servo_function_name(46.0), Some("Telemetry receive"));
    }

    #[test]
    fn a_function_the_specification_does_not_name_is_not_named() {
        assert_eq!(servo_function_name(47.0), None);
        assert_eq!(servo_function_name(-1.0), None);
    }

    #[test]
    fn the_motors_are_numbered_from_one() {
        assert_eq!(motor_number(69.0), Some(1));
        assert_eq!(motor_number(70.0), Some(2));
        assert_eq!(motor_number(128.0), Some(60));
        assert_eq!(motor_number(1.0), None);
    }
}
