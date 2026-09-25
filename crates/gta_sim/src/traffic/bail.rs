//! A shot into a traffic car's cabin scares its driver (GDD §5.2 Q-Б): the car brakes, the driver
//! gets out and flees (`advance_traffic`), and the shot is a shooting (`DriverScared`).

use super::{DriverScared, TrafficCar, TrafficMode};
use crate::vehicle::CabinHit;
use bevy::prelude::*;

pub(super) fn bail_out(
    mut hits: MessageReader<CabinHit>,
    mut scared: MessageWriter<DriverScared>,
    mut cars: Query<&mut TrafficCar>,
) {
    for hit in hits.read() {
        let Ok(mut car) = cars.get_mut(hit.vehicle) else {
            continue;
        };
        // The first hit wins; a car already bailing (or with no driver aboard) ignores the rest.
        if !matches!(car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic) {
            continue;
        }
        car.mode = TrafficMode::Bailing {
            attack: Some(hit.attack),
            shooter: Some(hit.shooter),
        };
        scared.write(DriverScared {
            shooter: hit.shooter,
            attack: hit.attack,
            vehicle: hit.vehicle,
        });
    }
}
