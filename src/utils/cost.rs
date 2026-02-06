use crate::db::edge::{
    Access, Bicycle, Cycleway, EdgePoint, Footway, Highway, Oneway, Smoothness, SourceOrTarget,
    Surface, Tunnel,
};
use crate::db::utils::distance_meters;
use crate::utils::elevation;

pub trait H: Send {
    fn get_cost(&self, edge: &EdgePoint) -> f64;
    fn get_max_point(&self) -> i64;

    fn h(&self, start_point: &EdgePoint, goal: &EdgePoint) -> f64 {
        let (goal_lon, goal_lat) = if SourceOrTarget::Source == goal.direction {
            (goal.lon1, goal.lat1)
        } else {
            (goal.lon2, goal.lat2)
        };
        let (start_lon, start_lat) = if SourceOrTarget::Source == start_point.direction {
            (start_point.lon1, start_point.lat1)
        } else {
            (start_point.lon2, start_point.lat2)
        };
        let distance = distance_meters(start_lat, start_lon, goal_lat, goal_lon);

        distance * 1.41
    }
}

pub fn get_h_moyen() -> Box<dyn H> {
    Box::new(HMoyen {})
}

pub fn get_h_bigger_selection() -> Box<dyn H> {
    Box::new(HBiggerSelection {})
}

#[allow(dead_code)]
pub fn get_h_rapid() -> Box<dyn H> {
    Box::new(HRapid {})
}

pub struct HMoyen {}

impl H for HMoyen {
    fn get_max_point(&self) -> i64 {
        i64::MAX
    }

    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        get_cost(FastOrSafe::Safe, edge)
    }
}

pub struct HBiggerSelection {}

impl H for HBiggerSelection {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        let cost = if edge.highway == Some(Highway::Cycleway) {
            1.
        } else if edge.cyclestreet {
            1.
        } else if edge.cycleway == Some(Cycleway::Crossing) {
            1.
        } else if edge.cycleway == Some(Cycleway::Track)
            || edge.cycleway_both == Some(Cycleway::Track)
            || edge.cycleway_left == Some(Cycleway::Track)
            || edge.cycleway_right == Some(Cycleway::Track)
        {
            1. / 0.9
        } else if edge.cycleway == Some(Cycleway::Lane)
            || edge.cycleway_both == Some(Cycleway::Lane)
            || edge.cycleway_left == Some(Cycleway::Lane)
            || edge.cycleway_right == Some(Cycleway::Lane)
        {
            1. / 0.8
        } else if edge.cycleway == Some(Cycleway::SharedLane)
            || edge.cycleway == Some(Cycleway::ShareBusway)
            || edge.cycleway_both == Some(Cycleway::SharedLane)
            || edge.cycleway_both == Some(Cycleway::ShareBusway)
            || edge.cycleway_left == Some(Cycleway::SharedLane)
            || edge.cycleway_left == Some(Cycleway::ShareBusway)
            || edge.cycleway_right == Some(Cycleway::SharedLane)
            || edge.cycleway_right == Some(Cycleway::ShareBusway)
        {
            1. / 0.7
        } else if edge.highway == Some(Highway::Unclassified) {
            1. / 0.7
        } else if edge.bicycle == Some(Bicycle::Designated) {
            1. / 0.7
        } else {
            1. / 0.1
        };

        cost
    }

    fn get_max_point(&self) -> i64 {
        10_000
    }
}

pub struct HRapid {}

impl H for HRapid {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        get_cost(FastOrSafe::Fast, edge)
    }

    fn get_max_point(&self) -> i64 {
        1000000000000000000
    }
}

enum FastOrSafe {
    Fast,
    Safe,
}

fn get_cost(fast_or_safe: FastOrSafe, edge: &EdgePoint) -> f64 {
    // if the target is the source we are reverse of the edge
    if SourceOrTarget::Source == edge.direction
        && (edge.oneway == Some(Oneway::Yes)
            && edge.oneway_bicycle != Some(Oneway::No)
            && edge.cycleway_left_oneway != Some(Oneway::No)
            && edge.cycleway_right_oneway != Some(Oneway::No)
            || edge.cycleway_left == Some(Cycleway::Snow))
    {
        return 1. / 0.0005;
    }

    if edge.winter_service_no && edge.snow {
        return 1. / 0.0001;
    }

    let bicycle = edge.bicycle.clone();
    let highway = edge.highway.clone();
    let cycleway = edge.cycleway.clone();
    let surface = edge.surface.clone();
    let smoothness = edge.smoothness.clone();
    let access = edge.access.clone();

    let mut cost: f64 = if bicycle == Some(Bicycle::No) {
        return 1. / 0.0001;
    } else if access == Some(Access::Private) || access == Some(Access::No) || edge.informal {
        if bicycle == Some(Bicycle::Yes) || bicycle == Some(Bicycle::Designated) {
            1. / 0.4
        } else {
            return 1. / 0.0001;
        }
    } else if highway == Some(Highway::Proposed)
        || edge.abandoned
        || highway == Some(Highway::Motorway)
    {
        return 1. / 0.0001;
    } else if access == Some(Access::Customers) {
        1. / 0.2
    } else if edge.informal {
        1. / 0.05
    } else if highway == Some(Highway::Steps) {
        if bicycle == Some(Bicycle::Yes) || bicycle == Some(Bicycle::Designated) {
            1. / 0.06
        } else {
            return 1. / 0.0001;
        }
    } else if highway == Some(Highway::Path)
        && (bicycle == Some(Bicycle::Dismount) || bicycle == Some(Bicycle::Discouraged))
    {
        1. / 0.1
    } else if bicycle == Some(Bicycle::Discouraged) {
        1. / 0.1
    } else if edge.routing_bicycle_use_sidepath {
        1. / 0.1
    } else if highway == Some(Highway::Cycleway) {
        if surface == Some(Surface::FineGravel) || surface == Some(Surface::Gravel) {
            1. / 0.75
        } else if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1.
        }
    } else if edge.cyclestreet {
        0.95
    } else if cycleway == Some(Cycleway::Track) || edge.cycleway_both == Some(Cycleway::Track) {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1.
        }
    } else if edge.cycleway_left == Some(Cycleway::Track)
        && (SourceOrTarget::Source == edge.direction
            || edge.cycleway_left_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1.
        }
    } else if edge.cycleway_right == Some(Cycleway::Track)
        && (SourceOrTarget::Target == edge.direction
            || edge.cycleway_right_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1.
        }
    } else if cycleway == Some(Cycleway::Lane) || edge.cycleway_both == Some(Cycleway::Lane) {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1. / 0.9
        }
    } else if edge.cycleway_left == Some(Cycleway::Lane)
        && (SourceOrTarget::Source == edge.direction
            || edge.cycleway_left_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1. / 0.9
        }
    } else if edge.cycleway_right == Some(Cycleway::Lane)
        && (SourceOrTarget::Target == edge.direction
            || edge.cycleway_right_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing) || smoothness == Some(Smoothness::Bad) {
            1. / 0.5
        } else {
            1. / 0.9
        }
    } else if highway == Some(Highway::Footway) {
        if bicycle == Some(Bicycle::Yes) || bicycle == Some(Bicycle::Designated) {
            if edge.footway == Some(Footway::Sidewalk) {
                1. / 0.4
            } else {
                1. / 0.7
            }
        } else if bicycle == Some(Bicycle::Dismount) {
            if edge.tunnel == Some(Tunnel::Yes) {
                1. / 0.2
            } else {
                1. / 0.3
            }
        } else {
            return 1. / 0.1;
        }
    } else if cycleway == Some(Cycleway::SharedLane)
        || cycleway == Some(Cycleway::ShareBusway)
        || edge.cycleway_both == Some(Cycleway::SharedLane)
        || edge.cycleway_both == Some(Cycleway::ShareBusway)
    {
        if cycleway == Some(Cycleway::Crossing)
            || smoothness == Some(Smoothness::Bad)
            || surface == Some(Surface::Sett)
        {
            1. / 0.5
        } else {
            1. / 0.7
        }
    } else if (edge.cycleway_left == Some(Cycleway::SharedLane)
        || edge.cycleway_left == Some(Cycleway::ShareBusway))
        && (SourceOrTarget::Source == edge.direction
            || edge.cycleway_left_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing)
            || smoothness == Some(Smoothness::Bad)
            || surface == Some(Surface::Sett)
        {
            1. / 0.5
        } else {
            1. / 0.7
        }
    } else if (edge.cycleway_right == Some(Cycleway::SharedLane)
        || edge.cycleway_right == Some(Cycleway::ShareBusway))
        && (SourceOrTarget::Target == edge.direction
            || edge.cycleway_right_oneway == Some(Oneway::No))
    {
        if cycleway == Some(Cycleway::Crossing)
            || smoothness == Some(Smoothness::Bad)
            || surface == Some(Surface::Sett)
        {
            1. / 0.5
        } else {
            1. / 0.7
        }
    } else if highway == Some(Highway::Residential) {
        if surface == Some(Surface::Sett) || surface == Some(Surface::Cobblestone) {
            1. / 0.4
        } else if bicycle == Some(Bicycle::Yes) || bicycle == Some(Bicycle::Designated) {
            1. / 0.85
        } else {
            1. / 0.6
        }
    } else if highway == Some(Highway::Unclassified) {
        1. / 0.5
    } else if highway == Some(Highway::Tertiary) {
        if surface == Some(Surface::Sett) || surface == Some(Surface::Cobblestone) {
            1. / 0.3
        } else if edge.in_bicycle_route {
            1. / 0.60
        } else {
            1. / 0.5
        }
    } else if highway == Some(Highway::Service) {
        if surface == Some(Surface::Chipseal) {
            1. / 0.2
        } else {
            1. / 0.3
        }
    } else if highway == Some(Highway::Secondary) {
        if surface == Some(Surface::Sett) || surface == Some(Surface::Cobblestone) {
            1. / 0.3
        } else if edge.in_bicycle_route {
            1. / 0.60
        } else {
            1. / 0.4
        }
    } else if edge.in_bicycle_route {
        1. / 0.60
    } else if bicycle == Some(Bicycle::Yes) || bicycle == Some(Bicycle::Designated) {
        1. / 0.4
    } else if highway == Some(Highway::SecondaryLink) {
        1. / 0.4
    } else if highway == Some(Highway::Primary) {
        1. / 0.3
    } else if highway == Some(Highway::Trunk) {
        1. / 0.3
    } else if highway.is_some() {
        1. / 0.3
    } else if highway == Some(Highway::Footway) {
        1. / 0.1
    } else {
        1. / 0.05
    };

    if edge.road_work {
        cost *= 3.;
    }

    cost = match fast_or_safe {
        FastOrSafe::Fast => 1. + cost.log(20.),
        FastOrSafe::Safe => {
            let slope_cost_bonus = elevation::get_edge_slope_cost(edge);
            cost + slope_cost_bonus
        }
    };

    let score = match edge.score {
        Some(score) => {
            if score == -1.0 {
                1.
            } else if score == 0. {
                0.01
            } else {
                score
            }
        }
        None => 1.,
    };
    cost / score
}
